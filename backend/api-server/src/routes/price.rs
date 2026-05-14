use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{Json, Router, extract::State, routing::get};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::warn;

use crate::services::defillama;
use crate::state::AppState;

static COINGECKO_API: &str = "https://api.coingecko.com/api/v3";

static SYMBOL_TO_ID: &[(&str, &str)] = &[
    ("ETH", "ethereum"),
    ("USDC", "usd-coin"),
    ("BTC", "bitcoin"),
    ("USDT", "tether"),
    ("DAI", "dai"),
    ("WETH", "weth"),
    ("STETH", "staked-ether"),
    ("CBETH", "coinbase-wrapped-staked-eth"),
    ("BNB", "binancecoin"),
    ("MATIC", "matic-network"),
    ("POL", "matic-network"),
    ("ARB", "arbitrum"),
    ("OP", "optimism"),
    ("LINK", "chainlink"),
    ("UNI", "uniswap"),
    ("AAVE", "aave"),
    ("WBTC", "wrapped-bitcoin"),
];

fn resolve_id(symbol: &str) -> Option<&'static str> {
    let upper = symbol.to_uppercase();
    SYMBOL_TO_ID
        .iter()
        .find(|(s, _)| *s == upper)
        .map(|(_, id)| *id)
}

#[derive(Clone)]
pub struct PriceCache {
    inner: Arc<RwLock<CacheInner>>,
}

struct CacheInner {
    data: HashMap<String, CachedPrice>,
    last_fetch: Option<Instant>,
}

#[derive(Clone)]
struct CachedPrice {
    usd: f64,
    change_24h: f64,
}

impl PriceCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(CacheInner {
                data: HashMap::new(),
                last_fetch: None,
            })),
        }
    }

    async fn get_prices(&self, client: &reqwest::Client, symbols: &[String]) -> HashMap<String, CachedPrice> {
        let ttl = Duration::from_secs(30);
        let cache = self.inner.read().await;

        if let Some(last) = cache.last_fetch {
            if last.elapsed() < ttl {
                let mut result = HashMap::new();
                for sym in symbols {
                    if let Some(cached) = cache.data.get(&sym.to_uppercase()) {
                        result.insert(sym.to_uppercase(), cached.clone());
                    }
                }
                if result.len() == symbols.len() {
                    return result;
                }
            }
        }
        drop(cache);

        // Primary: DeFiLlama (free, no key, no rate limit)
        let mut result = HashMap::new();
        match defillama::get_prices(client, symbols).await {
            Ok(llama_prices) => {
                let mut cache = self.inner.write().await;
                for (sym, price) in &llama_prices {
                    let entry = CachedPrice {
                        usd: price.usd,
                        change_24h: 0.0, // DeFiLlama /current doesn't return 24h change
                    };
                    cache.data.insert(sym.clone(), entry.clone());
                    result.insert(sym.clone(), entry);
                }
                cache.last_fetch = Some(Instant::now());

                // If we got all symbols, return early
                if result.len() == symbols.len() {
                    return result;
                }
            }
            Err(e) => {
                warn!("DeFiLlama price fetch failed, falling back to CoinGecko: {}", e);
            }
        }

        // Fallback: CoinGecko for any symbols DeFiLlama missed
        let missing: Vec<String> = symbols
            .iter()
            .filter(|s| !result.contains_key(&s.to_uppercase()))
            .cloned()
            .collect();

        if missing.is_empty() {
            return result;
        }

        let ids: Vec<&str> = missing.iter().filter_map(|s| resolve_id(s)).collect();
        if ids.is_empty() {
            return result;
        }

        let ids_param = ids.join(",");
        let url = format!(
            "{}/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true",
            COINGECKO_API, ids_param
        );

        let fetched = match client.get(&url).send().await {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(v) => v,
                Err(_) => return self.merge_fallback(symbols, result).await,
            },
            Err(_) => return self.merge_fallback(symbols, result).await,
        };

        let mut cache = self.inner.write().await;
        for sym in &missing {
            let upper = sym.to_uppercase();
            if let Some(id) = resolve_id(&upper) {
                if let Some(coin) = fetched.get(id) {
                    let usd = coin.get("usd").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let change = coin
                        .get("usd_24h_change")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);
                    let entry = CachedPrice {
                        usd,
                        change_24h: change,
                    };
                    cache.data.insert(upper.clone(), entry.clone());
                    result.insert(upper, entry);
                }
            }
        }
        cache.last_fetch = Some(Instant::now());
        result
    }

    /// Get a single token's USD price. Returns None if unavailable.
    pub async fn get_usd_price(&self, client: &reqwest::Client, symbol: &str) -> Option<f64> {
        let prices = self.get_prices(client, &[symbol.to_string()]).await;
        prices.get(&symbol.to_uppercase()).map(|p| p.usd)
    }

    /// Get token price by contract address using DeFiLlama.
    pub async fn get_token_price_by_address(
        &self,
        client: &reqwest::Client,
        chain_id: u64,
        contract_address: &str,
    ) -> Option<f64> {
        match defillama::get_token_price_by_address(client, chain_id, contract_address).await {
            Ok(price) => Some(price),
            Err(e) => {
                warn!("DeFiLlama token price by address failed: {}", e);
                None
            }
        }
    }

    async fn merge_fallback(&self, symbols: &[String], mut existing: HashMap<String, CachedPrice>) -> HashMap<String, CachedPrice> {
        let cache = self.inner.read().await;
        for sym in symbols {
            let upper = sym.to_uppercase();
            if !existing.contains_key(&upper) {
                if let Some(cached) = cache.data.get(&upper) {
                    existing.insert(upper, cached.clone());
                }
            }
        }
        existing
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_prices))
        .route("/token", get(get_token_price))
        .route("/history", get(price_history))
}

#[derive(Deserialize)]
struct PriceQuery {
    tokens: String,
}

#[derive(Serialize)]
struct PriceResponse {
    prices: Vec<TokenPrice>,
}

#[derive(Serialize)]
struct TokenPrice {
    symbol: String,
    usd: f64,
    change_24h: f64,
}

async fn get_prices(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<PriceQuery>,
) -> Json<PriceResponse> {
    let symbols: Vec<String> = q.tokens.split(',').map(|s| s.trim().to_string()).take(20).collect();

    let cached = state.price_cache.get_prices(&state.http, &symbols).await;

    let prices = symbols
        .iter()
        .map(|sym| {
            let upper = sym.to_uppercase();
            if let Some(entry) = cached.get(&upper) {
                TokenPrice {
                    symbol: upper,
                    usd: entry.usd,
                    change_24h: entry.change_24h,
                }
            } else {
                TokenPrice {
                    symbol: upper,
                    usd: 0.0,
                    change_24h: 0.0,
                }
            }
        })
        .collect();

    Json(PriceResponse { prices })
}

#[derive(Deserialize)]
struct TokenPriceQuery {
    chain_id: u64,
    address: String,
}

#[derive(Serialize)]
struct TokenPriceResponse {
    chain_id: u64,
    address: String,
    usd: f64,
}

async fn get_token_price(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<TokenPriceQuery>,
) -> Result<Json<TokenPriceResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let price = state
        .price_cache
        .get_token_price_by_address(&state.http, q.chain_id, &q.address)
        .await
        .ok_or_else(|| {
            (
                axum::http::StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "token price not found" })),
            )
        })?;

    Ok(Json(TokenPriceResponse {
        chain_id: q.chain_id,
        address: q.address,
        usd: price,
    }))
}

#[derive(Deserialize)]
struct HistoryQuery {
    token: Option<String>,
    days: Option<u32>,
}

#[derive(Serialize)]
struct HistoryResponse {
    symbol: String,
    prices: Vec<[f64; 2]>,
}

async fn price_history(
    State(state): State<AppState>,
    axum::extract::Query(q): axum::extract::Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, (axum::http::StatusCode, Json<serde_json::Value>)> {
    let symbol = q.token.unwrap_or_else(|| "ETH".into());
    let days = q.days.unwrap_or(7).min(90);

    let id = resolve_id(&symbol).ok_or_else(|| {
        (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "unsupported token" })),
        )
    })?;

    let url = format!(
        "{}/coins/{}/market_chart?vs_currency=usd&days={}",
        COINGECKO_API, id, days
    );

    let resp = state.http.get(&url).send()
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| {
            (
                axum::http::StatusCode::BAD_GATEWAY,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
        })?;

    let prices = resp
        .get("prices")
        .and_then(|p| p.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|pair| {
                    let a = pair.as_array()?;
                    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?])
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(Json(HistoryResponse {
        symbol: symbol.to_uppercase(),
        prices,
    }))
}
