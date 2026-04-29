use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{Json, Router, extract::State, routing::get};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

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

        let ids: Vec<&str> = symbols.iter().filter_map(|s| resolve_id(s)).collect();
        if ids.is_empty() {
            return HashMap::new();
        }

        let ids_param = ids.join(",");
        let url = format!(
            "{}/simple/price?ids={}&vs_currencies=usd&include_24hr_change=true",
            COINGECKO_API, ids_param
        );

        let fetched = match client.get(&url).send().await {
            Ok(resp) => match resp.json::<serde_json::Value>().await {
                Ok(v) => v,
                Err(_) => return self.fallback(symbols).await,
            },
            Err(_) => return self.fallback(symbols).await,
        };

        let mut cache = self.inner.write().await;
        let mut result = HashMap::new();

        for sym in symbols {
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

    async fn fallback(&self, symbols: &[String]) -> HashMap<String, CachedPrice> {
        let cache = self.inner.read().await;
        let mut result = HashMap::new();
        for sym in symbols {
            if let Some(cached) = cache.data.get(&sym.to_uppercase()) {
                result.insert(sym.to_uppercase(), cached.clone());
            }
        }
        result
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_prices))
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
