//! Covalent (GoldRush) API client for multi-token balance queries.
//!
//! Env: `COVALENT_API_KEY`
//!
//! Single request returns native coin + all ERC-20 balances with USD quotes.

use reqwest::Client;
use serde::Deserialize;
use tracing::warn;

use crate::retry::{retry_with_backoff, RetryConfig};

fn is_retryable_covalent_error(err: &String) -> bool {
    err.contains("request failed")
        || err.contains("timed out")
        || err.contains("connection")
        || err.contains("502")
        || err.contains("503")
        || err.contains("429")
}

const BASE_URL: &str = "https://api.covalenthq.com/v1";

pub fn chain_slug(chain_id: u64) -> Option<&'static str> {
    match chain_id {
        1 => Some("eth-mainnet"),
        8453 => Some("base-mainnet"),
        42161 => Some("arbitrum-mainnet"),
        10 => Some("optimism-mainnet"),
        56 => Some("bsc-mainnet"),
        137 => Some("matic-mainnet"),
        _ => None,
    }
}

pub fn is_testnet(chain_id: u64) -> bool {
    matches!(chain_id, 84532 | 11155111 | 421614 | 11155420 | 80002)
}

#[derive(Debug, Deserialize)]
struct CovalentResponse {
    data: Option<CovalentData>,
    error: bool,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CovalentData {
    address: String,
    chain_id: u64,
    items: Vec<CovalentBalanceItem>,
}

#[derive(Debug, Deserialize)]
struct CovalentBalanceItem {
    contract_decimals: Option<u32>,
    contract_ticker_symbol: Option<String>,
    contract_address: Option<String>,
    balance: Option<String>,
    quote: Option<f64>,
    native_token: Option<bool>,
    #[serde(rename = "type")]
    item_type: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenBalance {
    pub symbol: String,
    pub balance: String,
    pub balance_formatted: String,
    pub usd: String,
    pub contract_address: Option<String>,
    pub decimals: u32,
    pub native_token: bool,
}

fn format_units(raw: &str, decimals: u32) -> String {
    if raw == "0" || raw.is_empty() {
        return "0".into();
    }
    let value = match raw.parse::<u128>() {
        Ok(v) => v,
        Err(_) => return "0".into(),
    };
    if value == 0 {
        return "0".into();
    }
    let divisor = 10u128.pow(decimals);
    let whole = value / divisor;
    let frac = value % divisor;
    if frac == 0 {
        format!("{}", whole)
    } else {
        let frac_str = format!("{:0>width$}", frac, width = decimals as usize);
        let trimmed = frac_str.trim_end_matches('0');
        let display = if trimmed.len() > 6 { &trimmed[..6] } else { trimmed };
        format!("{}.{}", whole, display)
    }
}

// ─── Transaction History ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct CovalentTxResponse {
    data: Option<CovalentTxData>,
    error: bool,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CovalentTxData {
    items: Vec<CovalentTxItem>,
}

#[derive(Debug, Deserialize)]
struct CovalentTxItem {
    tx_hash: Option<String>,
    from_address: Option<String>,
    to_address: Option<String>,
    value: Option<String>,
    block_signed_at: Option<String>,
    successful: Option<bool>,
    gas_spent: Option<u64>,
    gas_quote: Option<f64>,
    value_quote: Option<f64>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct TransactionItem {
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    pub value: String,
    pub timestamp: String,
    pub status: String,
    pub gas_used: u64,
    pub token_symbol: String,
    pub value_quote: f64,
}

pub async fn get_transactions(
    http: &Client,
    api_key: &str,
    address: &str,
    chain_id: u64,
) -> Result<Vec<TransactionItem>, String> {
    if is_testnet(chain_id) {
        return Err(format!("Testnet chain {} not supported by Covalent — use RPC fallback", chain_id));
    }

    let slug = chain_slug(chain_id)
        .ok_or_else(|| format!("Unsupported chain_id: {}", chain_id))?;

    let url = format!(
        "{}/{}/address/{}/transactions_v3/?page-size=20",
        BASE_URL, slug, address
    );

    let http = http.clone();
    let url_clone = url.clone();
    let api_key = api_key.to_string();

    let body = retry_with_backoff(
        RetryConfig::conservative(),
        || {
            let http = http.clone();
            let url = url_clone.clone();
            let key = api_key.clone();
            async move {
                let resp = http
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .send()
                    .await
                    .map_err(|e| format!("Covalent tx request failed: {}", e))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    return Err(format!("Covalent API returned {}", status));
                }

                let body: CovalentTxResponse = resp
                    .json()
                    .await
                    .map_err(|e| format!("Covalent tx parse error: {}", e))?;

                if body.error {
                    return Err(format!(
                        "Covalent error: {}",
                        body.error_message.unwrap_or_default()
                    ));
                }

                Ok(body)
            }
        },
        is_retryable_covalent_error,
        "covalent_get_transactions",
    )
    .await?;

    let data = body.data.ok_or("Covalent returned no transaction data")?;

    let transactions: Vec<TransactionItem> = data
        .items
        .into_iter()
        .map(|item| TransactionItem {
            tx_hash: item.tx_hash.unwrap_or_default(),
            from: item.from_address.unwrap_or_default(),
            to: item.to_address.unwrap_or_default(),
            value: item.value.unwrap_or_else(|| "0".to_string()),
            timestamp: item.block_signed_at.unwrap_or_default(),
            status: if item.successful.unwrap_or(false) {
                "confirmed".to_string()
            } else {
                "failed".to_string()
            },
            gas_used: item.gas_spent.unwrap_or(0),
            token_symbol: "ETH".to_string(),
            value_quote: item.value_quote.unwrap_or(0.0),
        })
        .collect();

    Ok(transactions)
}

// ─── Balances ────────────────────────────────────────────────────────────────

pub async fn get_balances(
    http: &Client,
    api_key: &str,
    address: &str,
    chain_id: u64,
) -> Result<Vec<TokenBalance>, String> {
    if is_testnet(chain_id) {
        return Err(format!("Testnet chain {} not supported by Covalent — use RPC fallback", chain_id));
    }

    let slug = chain_slug(chain_id)
        .ok_or_else(|| format!("Unsupported chain_id: {}", chain_id))?;

    let url = format!(
        "{}/{}/address/{}/balances_v2/",
        BASE_URL, slug, address
    );

    let http = http.clone();
    let url_clone = url.clone();
    let api_key = api_key.to_string();

    let body = retry_with_backoff(
        RetryConfig::conservative(),
        || {
            let http = http.clone();
            let url = url_clone.clone();
            let key = api_key.clone();
            async move {
                let resp = http
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .send()
                    .await
                    .map_err(|e| format!("Covalent request failed: {}", e))?;

                if !resp.status().is_success() {
                    let status = resp.status();
                    return Err(format!("Covalent API returned {}", status));
                }

                let body: CovalentResponse = resp
                    .json()
                    .await
                    .map_err(|e| format!("Covalent parse error: {}", e))?;

                if body.error {
                    return Err(format!(
                        "Covalent error: {}",
                        body.error_message.unwrap_or_default()
                    ));
                }

                Ok(body)
            }
        },
        is_retryable_covalent_error,
        "covalent_get_balances",
    )
    .await?;

    let data = body.data.ok_or("Covalent returned no data")?;

    let mut tokens: Vec<TokenBalance> = Vec::new();

    for item in data.items {
        if item.item_type.as_deref() == Some("nft") {
            continue;
        }
        let raw_balance = item.balance.as_deref().unwrap_or("0");
        let is_native = item.native_token.unwrap_or(false);
        let is_zero = raw_balance == "0" || raw_balance.is_empty();
        if is_zero && !is_native {
            continue;
        }

        let decimals = item.contract_decimals.unwrap_or(18);
        let symbol = item
            .contract_ticker_symbol
            .unwrap_or_else(|| if is_native { "ETH".into() } else { "???".into() });
        let formatted = format_units(raw_balance, decimals);
        let usd = format!("{:.2}", item.quote.unwrap_or(0.0));

        tokens.push(TokenBalance {
            symbol,
            balance: raw_balance.to_string(),
            balance_formatted: formatted,
            usd,
            contract_address: item.contract_address,
            decimals,
            native_token: is_native,
        });
    }

    // Native token first
    tokens.sort_by(|a, b| b.native_token.cmp(&a.native_token));

    Ok(tokens)
}
