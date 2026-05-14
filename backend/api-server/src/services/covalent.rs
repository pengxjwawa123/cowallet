//! Covalent (GoldRush) API client for multi-token balance queries.
//!
//! Env: `COVALENT_API_KEY`
//!
//! Single request returns native coin + all ERC-20 balances with USD quotes.

use std::collections::HashMap;

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
        // Mainnets
        1 => Some("eth-mainnet"),
        8453 => Some("base-mainnet"),
        42161 => Some("arbitrum-mainnet"),
        10 => Some("optimism-mainnet"),
        56 => Some("bsc-mainnet"),
        137 => Some("matic-mainnet"),
        // Testnets
        11155111 => Some("eth-sepolia"),
        84532 => Some("base-sepolia-testnet"),
        421614 => Some("arbitrum-sepolia"),
        11155420 => Some("optimism-sepolia"),
        80002 => Some("polygon-amoy-testnet"),
        _ => None,
    }
}

pub fn is_testnet(chain_id: u64) -> bool {
    matches!(chain_id, 84532 | 11155111 | 421614 | 11155420 | 80002)
}

pub fn chain_display_name(chain_id: u64) -> &'static str {
    match chain_id {
        1 => "Ethereum",
        8453 => "Base",
        42161 => "Arbitrum One",
        10 => "Optimism",
        56 => "BNB Chain",
        137 => "Polygon",
        11155111 => "Ethereum Sepolia",
        84532 => "Base Sepolia",
        421614 => "Arbitrum Sepolia",
        11155420 => "Optimism Sepolia",
        80002 => "Polygon Amoy",
        _ => "Unknown",
    }
}

pub fn native_symbol(chain_id: u64) -> &'static str {
    match chain_id {
        137 | 80002 => "POL",
        56 => "BNB",
        _ => "ETH",
    }
}

#[derive(Debug, Deserialize)]
struct CovalentResponse {
    data: Option<CovalentData>,
    error: bool,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CovalentData {
    #[allow(dead_code)]
    address: Option<String>,
    chain_id: Option<u64>,
    items: Vec<CovalentBalanceItem>,
}

#[derive(Debug, Deserialize)]
struct CovalentBalanceItem {
    contract_decimals: Option<u32>,
    contract_name: Option<String>,
    contract_ticker_symbol: Option<String>,
    contract_address: Option<String>,
    balance: Option<String>,
    balance_24h: Option<String>,
    quote: Option<f64>,
    quote_24h: Option<f64>,
    quote_rate: Option<f64>,
    quote_rate_24h: Option<f64>,
    pretty_quote: Option<String>,
    #[serde(rename = "is_native_token")]
    native_token: Option<bool>,
    is_spam: Option<bool>,
    #[serde(rename = "type")]
    item_type: Option<String>,
    last_transferred_at: Option<String>,
    logo_urls: Option<CovalentLogoUrls>,
    chain_id: Option<u64>,
    chain_name: Option<String>,
    chain_display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct CovalentLogoUrls {
    token_logo_url: Option<String>,
    chain_logo_url: Option<String>,
}

/// All token balance fields needed for display and business logic.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TokenBalance {
    pub symbol: String,
    pub name: String,
    pub balance: String,
    pub balance_formatted: String,
    pub balance_24h: Option<String>,
    pub usd: String,
    pub usd_24h: Option<String>,
    pub quote_rate: Option<f64>,
    pub quote_rate_24h: Option<f64>,
    pub pretty_quote: Option<String>,
    pub contract_address: Option<String>,
    pub decimals: u32,
    pub native_token: bool,
    pub is_spam: bool,
    pub token_type: Option<String>,
    pub logo_url: Option<String>,
    pub chain_logo_url: Option<String>,
    pub chain_id: Option<u64>,
    pub chain_name: Option<String>,
    pub last_transferred_at: Option<String>,
}

pub fn format_value(raw: &str, decimals: u32) -> String {
    format_units(raw, decimals)
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
    pub chain_id: u64,
    pub chain_name: String,
}

pub async fn get_transactions(
    http: &Client,
    api_key: &str,
    address: &str,
    chain_id: u64,
) -> Result<Vec<TransactionItem>, String> {

    let slug = chain_slug(chain_id)
        .ok_or_else(|| format!("Unsupported chain_id: {}", chain_id))?;

    let url = format!(
        "{}/{}/address/{}/transactions_v3/?page-size=20",
        BASE_URL, slug, address
    );

    tracing::info!("[Covalent] get_transactions chain={} slug={} url={}", chain_id, slug, url);

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

                let status = resp.status();
                tracing::info!("[Covalent] get_transactions response status={}", status);

                if !status.is_success() {
                    let body_text = resp.text().await.unwrap_or_default();
                    tracing::error!("[Covalent] get_transactions error body: {}", body_text);
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
            token_symbol: native_symbol(chain_id).to_string(),
            value_quote: item.value_quote.unwrap_or(0.0),
            chain_id,
            chain_name: chain_display_name(chain_id).to_string(),
        })
        .collect();

    Ok(transactions)
}

/// Get transaction history across multiple chains by querying each chain in parallel
pub async fn get_all_chain_transactions(
    http: &Client,
    api_key: &str,
    address: &str,
    chain_ids: &[u64],
) -> Result<Vec<TransactionItem>, String> {
    use futures::future::join_all;

    let futures: Vec<_> = chain_ids
        .iter()
        .filter(|&&id| chain_slug(id).is_some())
        .map(|&chain_id| {
            let http = http.clone();
            let api_key = api_key.to_string();
            let address = address.to_string();
            async move {
                get_transactions(&http, &api_key, &address, chain_id).await
            }
        })
        .collect();

    let results = join_all(futures).await;

    let mut all_transactions = Vec::new();
    for (idx, result) in results.into_iter().enumerate() {
        match result {
            Ok(mut txs) => {
                all_transactions.append(&mut txs);
            }
            Err(e) => {
                let chain_id = chain_ids[idx];
                warn!("Failed to get transactions for chain {}: {}", chain_id, e);
            }
        }
    }

    all_transactions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

    Ok(all_transactions)
}

// ─── Balances ────────────────────────────────────────────────────────────────

pub async fn get_balances(
    http: &Client,
    api_key: &str,
    address: &str,
    chain_id: u64,
) -> Result<Vec<TokenBalance>, String> {

    let slug = chain_slug(chain_id)
        .ok_or_else(|| format!("Unsupported chain_id: {}", chain_id))?;

    let url = format!(
        "{}/{}/address/{}/balances_v2/",
        BASE_URL, slug, address
    );

    tracing::info!("[Covalent] get_balances chain={} slug={} url={}", chain_id, slug, url);

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

                let status = resp.status();
                tracing::info!("[Covalent] get_balances response status={}", status);

                if !status.is_success() {
                    let body_text = resp.text().await.unwrap_or_default();
                    tracing::error!("[Covalent] get_balances error body: {}", body_text);
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
    tracing::info!("[Covalent] get_balances chain={} got {} items", chain_id, data.items.len());

    let mut tokens: Vec<TokenBalance> = Vec::new();

    for item in data.items {
        if item.item_type.as_deref() == Some("nft") {
            continue;
        }
        if item.is_spam.unwrap_or(false) {
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
            .clone()
            .unwrap_or_else(|| if is_native { native_symbol(chain_id).into() } else { "???".into() });
        let name = item.contract_name.unwrap_or_else(|| symbol.clone());
        let formatted = format_units(raw_balance, decimals);
        let usd = format!("{:.2}", item.quote.unwrap_or(0.0));
        let usd_24h = item.quote_24h.map(|q| format!("{:.2}", q));

        let (logo_url, chain_logo_url) = match item.logo_urls {
            Some(logos) => (logos.token_logo_url, logos.chain_logo_url),
            None => (None, None),
        };

        tokens.push(TokenBalance {
            symbol,
            name,
            balance: raw_balance.to_string(),
            balance_formatted: formatted,
            balance_24h: item.balance_24h,
            usd,
            usd_24h,
            quote_rate: item.quote_rate,
            quote_rate_24h: item.quote_rate_24h,
            pretty_quote: item.pretty_quote,
            contract_address: item.contract_address,
            decimals,
            native_token: is_native,
            is_spam: false,
            token_type: item.item_type,
            logo_url,
            chain_logo_url,
            chain_id: Some(chain_id),
            chain_name: Some(chain_display_name(chain_id).to_string()),
            last_transferred_at: item.last_transferred_at,
        });
    }

    // Native token first
    tokens.sort_by(|a, b| b.native_token.cmp(&a.native_token));

    Ok(tokens)
}

// ─── Cross-Chain Balances (per-chain parallel) ──────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
pub struct ChainBalance {
    pub chain_id: u64,
    pub chain_name: String,
    pub tokens: Vec<TokenBalance>,
    pub total_usd: String,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct AllChainsBalance {
    pub address: String,
    pub chains: Vec<ChainBalance>,
    pub total_usd: String,
}

/// Get balances across multiple chains by querying each chain in parallel
pub async fn get_all_chain_balances(
    http: &Client,
    api_key: &str,
    address: &str,
    chain_ids: &[u64],
) -> Result<AllChainsBalance, String> {
    use futures::future::join_all;

    tracing::info!("[Covalent] get_all_chain_balances address={} chains={:?}", address, chain_ids);

    let futures: Vec<_> = chain_ids
        .iter()
        .filter(|&&id| chain_slug(id).is_some())
        .map(|&chain_id| {
            let http = http.clone();
            let api_key = api_key.to_string();
            let address = address.to_string();
            async move {
                let result = get_balances(&http, &api_key, &address, chain_id).await;
                (chain_id, result)
            }
        })
        .collect();

    let results = join_all(futures).await;

    let mut chains: Vec<ChainBalance> = Vec::new();
    let mut grand_total_usd = 0.0;

    for (chain_id, result) in results {
        match result {
            Ok(tokens) => {
                let chain_total: f64 = tokens
                    .iter()
                    .filter_map(|t| t.usd.parse::<f64>().ok())
                    .sum();
                grand_total_usd += chain_total;
                tracing::info!("[Covalent] chain {} OK: {} tokens, ${:.2}", chain_id, tokens.len(), chain_total);

                chains.push(ChainBalance {
                    chain_id,
                    chain_name: chain_display_name(chain_id).to_string(),
                    tokens,
                    total_usd: format!("{:.2}", chain_total),
                });
            }
            Err(e) => {
                tracing::error!("[Covalent] chain {} FAILED: {}", chain_id, e);
                warn!("Failed to get balances for chain {}: {}", chain_id, e);
            }
        }
    }

    chains.sort_by_key(|c| c.chain_id);

    Ok(AllChainsBalance {
        address: address.to_string(),
        chains,
        total_usd: format!("{:.2}", grand_total_usd),
    })
}

// ─── Allchains Single-Request Balances ──────────────────────────────────────

#[derive(Debug, Deserialize)]
struct AllchainsResponse {
    data: Option<AllchainsData>,
    error: Option<bool>,
    error_message: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AllchainsData {
    items: Vec<CovalentBalanceItem>,
}

/// Get balances across ALL chains in a single API call (Covalent allchains endpoint).
/// Falls back to per-chain parallel if this fails (e.g. free tier doesn't support it).
pub async fn get_allchains_balances(
    http: &Client,
    api_key: &str,
    address: &str,
) -> Result<AllChainsBalance, String> {
    let url = format!(
        "{}/allchains/address/{}/balances/",
        BASE_URL, address
    );

    tracing::info!("[Covalent] get_allchains_balances address={} url={}", address, url);

    let http_clone = http.clone();
    let url_clone = url.clone();
    let api_key_owned = api_key.to_string();

    let body = retry_with_backoff(
        RetryConfig::conservative(),
        || {
            let http = http_clone.clone();
            let url = url_clone.clone();
            let key = api_key_owned.clone();
            async move {
                let resp = http
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", key))
                    .send()
                    .await
                    .map_err(|e| format!("Covalent allchains request failed: {}", e))?;

                let status = resp.status();
                tracing::info!("[Covalent] allchains response status={}", status);

                if !status.is_success() {
                    let body_text = resp.text().await.unwrap_or_default();
                    tracing::error!("[Covalent] allchains error body: {}", body_text);
                    return Err(format!("Covalent allchains API returned {}", status));
                }

                let body: AllchainsResponse = resp
                    .json()
                    .await
                    .map_err(|e| format!("Covalent allchains parse error: {}", e))?;

                if body.error.unwrap_or(false) {
                    return Err(format!(
                        "Covalent allchains error: {}",
                        body.error_message.unwrap_or_default()
                    ));
                }

                Ok(body)
            }
        },
        is_retryable_covalent_error,
        "covalent_get_allchains_balances",
    )
    .await?;

    let data = body.data.ok_or("Covalent allchains returned no data")?;
    tracing::info!("[Covalent] allchains got {} items", data.items.len());

    // Group tokens by chain_id
    let mut chain_map: HashMap<u64, Vec<TokenBalance>> = HashMap::new();

    for item in data.items {
        if item.item_type.as_deref() == Some("nft") {
            continue;
        }
        if item.is_spam.unwrap_or(false) {
            continue;
        }

        let raw_balance = item.balance.as_deref().unwrap_or("0");
        let is_native = item.native_token.unwrap_or(false);
        let is_zero = raw_balance == "0" || raw_balance.is_empty();
        if is_zero && !is_native {
            continue;
        }

        let item_chain_id = item.chain_id.unwrap_or(1);
        let decimals = item.contract_decimals.unwrap_or(18);
        let symbol = item
            .contract_ticker_symbol
            .clone()
            .unwrap_or_else(|| if is_native { native_symbol(item_chain_id).into() } else { "???".into() });
        let name = item.contract_name.unwrap_or_else(|| symbol.clone());
        let formatted = format_units(raw_balance, decimals);
        let usd = format!("{:.2}", item.quote.unwrap_or(0.0));
        let usd_24h = item.quote_24h.map(|q| format!("{:.2}", q));
        let item_chain_name = item.chain_display_name
            .or(item.chain_name)
            .unwrap_or_else(|| chain_display_name(item_chain_id).to_string());

        let (logo_url, chain_logo_url) = match item.logo_urls {
            Some(logos) => (logos.token_logo_url, logos.chain_logo_url),
            None => (None, None),
        };

        let token = TokenBalance {
            symbol,
            name,
            balance: raw_balance.to_string(),
            balance_formatted: formatted,
            balance_24h: item.balance_24h,
            usd,
            usd_24h,
            quote_rate: item.quote_rate,
            quote_rate_24h: item.quote_rate_24h,
            pretty_quote: item.pretty_quote,
            contract_address: item.contract_address,
            decimals,
            native_token: is_native,
            is_spam: false,
            token_type: item.item_type,
            logo_url,
            chain_logo_url: chain_logo_url.clone(),
            chain_id: Some(item_chain_id),
            chain_name: Some(item_chain_name.clone()),
            last_transferred_at: item.last_transferred_at,
        };

        chain_map.entry(item_chain_id).or_default().push(token);
    }

    let mut chains: Vec<ChainBalance> = Vec::new();
    let mut grand_total_usd = 0.0;

    for (chain_id, mut tokens) in chain_map {
        tokens.sort_by(|a, b| b.native_token.cmp(&a.native_token));
        let chain_total: f64 = tokens
            .iter()
            .filter_map(|t| t.usd.parse::<f64>().ok())
            .sum();
        grand_total_usd += chain_total;

        chains.push(ChainBalance {
            chain_id,
            chain_name: tokens.first()
                .and_then(|t| t.chain_name.clone())
                .unwrap_or_else(|| chain_display_name(chain_id).to_string()),
            tokens,
            total_usd: format!("{:.2}", chain_total),
        });
    }

    chains.sort_by_key(|c| c.chain_id);

    Ok(AllChainsBalance {
        address: address.to_string(),
        chains,
        total_usd: format!("{:.2}", grand_total_usd),
    })
}
