//! DEX aggregator client using 0x Swap API.
//!
//! Provides token swap quotes and transaction building for multi-chain swaps.
//! Env: `ZEROX_API_KEY` (optional, for paid tier)

use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::retry::{retry_with_backoff, RetryConfig};

fn is_retryable_dex_error(err: &String) -> bool {
    err.contains("request failed")
        || err.contains("timed out")
        || err.contains("connection")
        || err.contains("502")
        || err.contains("503")
        || err.contains("429")
}

/// Native ETH address placeholder used by 0x API
pub const NATIVE_TOKEN_ADDRESS: &str = "0xEeeeeEeeeEeEeeEeEeEeeEEEeeeeEeeeeeeeEEeE";

/// Get the 0x API base URL for a given chain
fn api_base_url(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("https://api.0x.org"),
        8453 => Some("https://base.api.0x.org"),
        42161 => Some("https://arbitrum.api.0x.org"),
        10 => Some("https://optimism.api.0x.org"),
        137 => Some("https://polygon.api.0x.org"),
        56 => Some("https://bsc.api.0x.org"),
        _ => None,
    }
}

/// Get the well-known token address for a symbol on a given chain.
/// Returns the 0x native placeholder for native gas tokens.
pub fn token_address(symbol: &str, chain_id: u64) -> Option<&'static str> {
    let s = symbol.to_uppercase();
    match (s.as_str(), chain_id) {
        // Native tokens
        ("ETH", 1 | 8453 | 42161 | 10) => Some(NATIVE_TOKEN_ADDRESS),
        ("BNB", 56) => Some(NATIVE_TOKEN_ADDRESS),
        ("POL" | "MATIC", 137) => Some(NATIVE_TOKEN_ADDRESS),
        // WETH
        ("WETH", 1) => Some("0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"),
        ("WETH", 8453) => Some("0x4200000000000000000000000000000000000006"),
        ("WETH", 42161) => Some("0x82aF49447D8a07e3bd95BD0d56f35241523fBab1"),
        ("WETH", 10) => Some("0x4200000000000000000000000000000000000006"),
        // USDC
        ("USDC", 1) => Some("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"),
        ("USDC", 8453) => Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
        ("USDC", 42161) => Some("0xaf88d065e77c8cC2239327C5EDb3A432268e5831"),
        ("USDC", 10) => Some("0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85"),
        ("USDC", 137) => Some("0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"),
        ("USDC", 56) => Some("0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d"),
        // USDT
        ("USDT", 1) => Some("0xdAC17F958D2ee523a2206206994597C13D831ec7"),
        ("USDT", 8453) => Some("0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2"),
        ("USDT", 42161) => Some("0xFd086bC7CD5C481DCC9C85ebE478A1C0b69FCbb9"),
        ("USDT", 10) => Some("0x94b008aA00579c1307B0EF2c499aD98a8ce58e58"),
        ("USDT", 137) => Some("0xc2132D05D31c914a87C6611C10748AEb04B58e8F"),
        ("USDT", 56) => Some("0x55d398326f99059fF775485246999027B3197955"),
        // DAI
        ("DAI", 1) => Some("0x6B175474E89094C44Da98b954EedeAC495271d0F"),
        ("DAI", 8453) => Some("0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
        ("DAI", 42161) => Some("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1"),
        ("DAI", 10) => Some("0xDA10009cBd5D07dd0CeCc66161FC93D7c9000da1"),
        // WBNB
        ("WBNB", 56) => Some("0xbb4CdB9CBd36B01bD1cBaEBF2De08d9173bc095c"),
        _ => None,
    }
}

/// Get token decimals for a known symbol
pub fn token_decimals(symbol: &str) -> u32 {
    match symbol.to_uppercase().as_str() {
        "USDC" | "USDT" => 6,
        _ => 18,
    }
}

// ─── Response types ─────────────────────────────────────────────────────────

/// Quote response from 0x API
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZeroxPriceResponse {
    buy_amount: Option<String>,
    sell_amount: Option<String>,
    price: Option<String>,
    estimated_price_impact: Option<String>,
    estimated_gas: Option<String>,
    gas_price: Option<String>,
    sources: Option<Vec<ZeroxSource>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ZeroxSource {
    name: Option<String>,
    proportion: Option<String>,
}

/// Quote response from 0x /swap/v1/quote (includes calldata)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ZeroxQuoteResponse {
    buy_amount: Option<String>,
    sell_amount: Option<String>,
    price: Option<String>,
    estimated_price_impact: Option<String>,
    estimated_gas: Option<String>,
    gas_price: Option<String>,
    to: Option<String>,
    data: Option<String>,
    value: Option<String>,
    sources: Option<Vec<ZeroxSource>>,
    allowance_target: Option<String>,
}

/// Our normalized swap quote
#[derive(Debug, Clone, Serialize)]
pub struct SwapQuote {
    pub sell_token: String,
    pub buy_token: String,
    pub sell_amount: String,
    pub buy_amount: String,
    pub price: String,
    pub price_impact: Option<String>,
    pub estimated_gas: String,
    pub gas_price: Option<String>,
    pub sources: Vec<String>,
    pub chain_id: u64,
}

/// Our normalized swap transaction data
#[derive(Debug, Clone, Serialize)]
pub struct SwapTransaction {
    pub to: String,
    pub data: String,
    pub value: String,
    pub gas_estimate: String,
    pub sell_token: String,
    pub buy_token: String,
    pub sell_amount: String,
    pub buy_amount: String,
    pub price: String,
    pub allowance_target: Option<String>,
    pub chain_id: u64,
}

// ─── Public API ─────────────────────────────────────────────────────────────

/// Get a swap price quote (no calldata, lower rate limit requirements)
pub async fn get_quote(
    http: &Client,
    api_key: Option<&str>,
    chain_id: u64,
    sell_token: &str,
    buy_token: &str,
    sell_amount: &str,
) -> Result<SwapQuote, String> {
    let base_url = api_base_url(chain_id)
        .ok_or_else(|| format!("Unsupported chain for swaps: {}", chain_id))?;

    let url = format!("{}/swap/v1/price", base_url);

    tracing::info!(
        "[DEX] get_quote chain={} sell={} buy={} amount={}",
        chain_id, sell_token, buy_token, sell_amount
    );

    let http = http.clone();
    let url_clone = url.clone();
    let sell_token_owned = sell_token.to_string();
    let buy_token_owned = buy_token.to_string();
    let sell_amount_owned = sell_amount.to_string();
    let api_key_owned = api_key.map(|s| s.to_string());

    let resp = retry_with_backoff(
        RetryConfig::conservative(),
        || {
            let http = http.clone();
            let url = url_clone.clone();
            let sell = sell_token_owned.clone();
            let buy = buy_token_owned.clone();
            let amount = sell_amount_owned.clone();
            let key = api_key_owned.clone();
            async move {
                let mut req = http
                    .get(&url)
                    .query(&[
                        ("sellToken", sell.as_str()),
                        ("buyToken", buy.as_str()),
                        ("sellAmount", amount.as_str()),
                    ]);

                if let Some(ref k) = key {
                    req = req.header("0x-api-key", k.as_str());
                }

                let response = req.send().await
                    .map_err(|e| format!("0x API request failed: {}", e))?;

                let status = response.status();
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    tracing::error!("[DEX] price API error {}: {}", status, body);
                    return Err(format!("0x API returned {}: {}", status, body));
                }

                let body: ZeroxPriceResponse = response.json().await
                    .map_err(|e| format!("0x API parse error: {}", e))?;

                Ok(body)
            }
        },
        is_retryable_dex_error,
        "dex_get_quote",
    )
    .await?;

    let sources: Vec<String> = resp.sources
        .unwrap_or_default()
        .into_iter()
        .filter(|s| {
            s.proportion.as_deref().unwrap_or("0") != "0"
        })
        .filter_map(|s| s.name)
        .collect();

    Ok(SwapQuote {
        sell_token: sell_token.to_string(),
        buy_token: buy_token.to_string(),
        sell_amount: resp.sell_amount.unwrap_or_else(|| sell_amount.to_string()),
        buy_amount: resp.buy_amount.unwrap_or_else(|| "0".to_string()),
        price: resp.price.unwrap_or_else(|| "0".to_string()),
        price_impact: resp.estimated_price_impact,
        estimated_gas: resp.estimated_gas.unwrap_or_else(|| "200000".to_string()),
        gas_price: resp.gas_price,
        sources,
        chain_id,
    })
}

/// Build a swap transaction with calldata for on-chain execution
pub async fn build_swap_tx(
    http: &Client,
    api_key: Option<&str>,
    chain_id: u64,
    sell_token: &str,
    buy_token: &str,
    sell_amount: &str,
    slippage_percent: f64,
    taker_address: &str,
) -> Result<SwapTransaction, String> {
    let base_url = api_base_url(chain_id)
        .ok_or_else(|| format!("Unsupported chain for swaps: {}", chain_id))?;

    let url = format!("{}/swap/v1/quote", base_url);
    let slippage_bps = format!("{:.4}", slippage_percent / 100.0);

    tracing::info!(
        "[DEX] build_swap_tx chain={} sell={} buy={} amount={} slippage={}% taker={}",
        chain_id, sell_token, buy_token, sell_amount, slippage_percent, taker_address
    );

    let http = http.clone();
    let url_clone = url.clone();
    let sell_token_owned = sell_token.to_string();
    let buy_token_owned = buy_token.to_string();
    let sell_amount_owned = sell_amount.to_string();
    let slippage_owned = slippage_bps.clone();
    let taker_owned = taker_address.to_string();
    let api_key_owned = api_key.map(|s| s.to_string());

    let resp = retry_with_backoff(
        RetryConfig::conservative(),
        || {
            let http = http.clone();
            let url = url_clone.clone();
            let sell = sell_token_owned.clone();
            let buy = buy_token_owned.clone();
            let amount = sell_amount_owned.clone();
            let slippage = slippage_owned.clone();
            let taker = taker_owned.clone();
            let key = api_key_owned.clone();
            async move {
                let mut req = http
                    .get(&url)
                    .query(&[
                        ("sellToken", sell.as_str()),
                        ("buyToken", buy.as_str()),
                        ("sellAmount", amount.as_str()),
                        ("slippagePercentage", slippage.as_str()),
                        ("takerAddress", taker.as_str()),
                    ]);

                if let Some(ref k) = key {
                    req = req.header("0x-api-key", k.as_str());
                }

                let response = req.send().await
                    .map_err(|e| format!("0x quote request failed: {}", e))?;

                let status = response.status();
                if !status.is_success() {
                    let body = response.text().await.unwrap_or_default();
                    tracing::error!("[DEX] quote API error {}: {}", status, body);
                    return Err(format!("0x quote API returned {}: {}", status, body));
                }

                let body: ZeroxQuoteResponse = response.json().await
                    .map_err(|e| format!("0x quote parse error: {}", e))?;

                Ok(body)
            }
        },
        is_retryable_dex_error,
        "dex_build_swap_tx",
    )
    .await?;

    Ok(SwapTransaction {
        to: resp.to.unwrap_or_default(),
        data: resp.data.unwrap_or_default(),
        value: resp.value.unwrap_or_else(|| "0".to_string()),
        gas_estimate: resp.estimated_gas.unwrap_or_else(|| "200000".to_string()),
        sell_token: sell_token.to_string(),
        buy_token: buy_token.to_string(),
        sell_amount: resp.sell_amount.unwrap_or_else(|| sell_amount.to_string()),
        buy_amount: resp.buy_amount.unwrap_or_else(|| "0".to_string()),
        price: resp.price.unwrap_or_else(|| "0".to_string()),
        allowance_target: resp.allowance_target,
        chain_id,
    })
}

/// Convert a human-readable amount to raw token units (wei/smallest unit)
pub fn amount_to_raw(amount: &str, decimals: u32) -> Result<String, String> {
    let amt: f64 = amount.parse()
        .map_err(|_| format!("Invalid amount: {}", amount))?;
    if amt <= 0.0 {
        return Err("Amount must be positive".to_string());
    }
    let raw = amt * 10f64.powi(decimals as i32);
    Ok(format!("{:.0}", raw))
}

/// Convert raw token units to human-readable amount
pub fn raw_to_amount(raw: &str, decimals: u32) -> String {
    let value: f64 = raw.parse().unwrap_or(0.0);
    let amount = value / 10f64.powi(decimals as i32);
    if decimals <= 6 {
        format!("{:.2}", amount)
    } else {
        format!("{:.6}", amount)
    }
}
