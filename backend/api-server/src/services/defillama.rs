//! DeFiLlama price API client — free, no API key required.
//!
//! Endpoint: `https://coins.llama.fi/prices/current/{coins}`
//! Coin format: `coingecko:{id}` or `{chain}:{tokenAddress}`

use std::collections::HashMap;

use reqwest::Client;
use serde::Deserialize;
use tracing::warn;

const LLAMA_BASE: &str = "https://coins.llama.fi";

static SYMBOL_TO_LLAMA_ID: &[(&str, &str)] = &[
    ("ETH", "coingecko:ethereum"),
    ("BTC", "coingecko:bitcoin"),
    ("USDC", "coingecko:usd-coin"),
    ("USDT", "coingecko:tether"),
    ("DAI", "coingecko:dai"),
    ("WETH", "coingecko:weth"),
    ("STETH", "coingecko:staked-ether"),
    ("CBETH", "coingecko:coinbase-wrapped-staked-eth"),
    ("BNB", "coingecko:binancecoin"),
    ("MATIC", "coingecko:matic-network"),
    ("POL", "coingecko:matic-network"),
    ("ARB", "coingecko:arbitrum"),
    ("OP", "coingecko:optimism"),
    ("LINK", "coingecko:chainlink"),
    ("UNI", "coingecko:uniswap"),
    ("AAVE", "coingecko:aave"),
    ("CRV", "coingecko:curve-dao-token"),
    ("LDO", "coingecko:lido-dao"),
    ("RETH", "coingecko:rocket-pool-eth"),
    ("WBTC", "coingecko:wrapped-bitcoin"),
];

pub fn resolve_llama_id(symbol: &str) -> Option<&'static str> {
    let upper = symbol.to_uppercase();
    SYMBOL_TO_LLAMA_ID
        .iter()
        .find(|(s, _)| *s == upper)
        .map(|(_, id)| *id)
}

/// Resolve a token to its DeFiLlama coin identifier by contract address + chain.
pub fn token_by_address(chain_id: u64, contract_address: &str) -> Option<String> {
    let chain = match chain_id {
        1 => "ethereum",
        8453 => "base",
        42161 => "arbitrum",
        10 => "optimism",
        56 => "bsc",
        137 => "polygon",
        _ => return None,
    };
    Some(format!("{}:{}", chain, contract_address))
}

#[derive(Debug, Deserialize)]
struct LlamaResponse {
    coins: HashMap<String, LlamaCoin>,
}

#[derive(Debug, Deserialize)]
struct LlamaCoin {
    price: f64,
    symbol: Option<String>,
    confidence: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct LlamaPrice {
    pub usd: f64,
    pub confidence: f64,
}

/// Fetch current prices from DeFiLlama for a list of symbols.
pub async fn get_prices(
    http: &Client,
    symbols: &[String],
) -> Result<HashMap<String, LlamaPrice>, String> {
    let coin_ids: Vec<(&str, &str)> = symbols
        .iter()
        .filter_map(|sym| {
            let upper = sym.to_uppercase();
            resolve_llama_id(&upper).map(|id| {
                let sym_ref: &str = SYMBOL_TO_LLAMA_ID
                    .iter()
                    .find(|(_, lid)| *lid == id)
                    .map(|(s, _)| *s)
                    .unwrap_or("");
                (sym_ref, id)
            })
        })
        .collect();

    if coin_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let ids_param: String = coin_ids.iter().map(|(_, id)| *id).collect::<Vec<_>>().join(",");
    let url = format!("{}/prices/current/{}", LLAMA_BASE, ids_param);

    let resp = http
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("DeFiLlama request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("DeFiLlama returned {}", resp.status()));
    }

    let body: LlamaResponse = resp
        .json()
        .await
        .map_err(|e| format!("DeFiLlama parse error: {}", e))?;

    let mut result = HashMap::new();
    for (sym, id) in &coin_ids {
        if let Some(coin) = body.coins.get(*id) {
            result.insert(
                sym.to_string(),
                LlamaPrice {
                    usd: coin.price,
                    confidence: coin.confidence.unwrap_or(0.99),
                },
            );
        }
    }

    Ok(result)
}

/// Fetch price for a single token by contract address on a specific chain.
pub async fn get_token_price_by_address(
    http: &Client,
    chain_id: u64,
    contract_address: &str,
) -> Result<f64, String> {
    let coin_id = token_by_address(chain_id, contract_address)
        .ok_or_else(|| format!("Unsupported chain_id {} for DeFiLlama", chain_id))?;

    let url = format!("{}/prices/current/{}", LLAMA_BASE, coin_id);

    let resp = http
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
        .map_err(|e| format!("DeFiLlama request failed: {}", e))?;

    if !resp.status().is_success() {
        return Err(format!("DeFiLlama returned {}", resp.status()));
    }

    let body: LlamaResponse = resp
        .json()
        .await
        .map_err(|e| format!("DeFiLlama parse error: {}", e))?;

    body.coins
        .get(&coin_id)
        .map(|c| c.price)
        .ok_or_else(|| "Token not found on DeFiLlama".to_string())
}

/// Batch fetch prices for multiple contract addresses across chains.
pub async fn get_batch_token_prices(
    http: &Client,
    tokens: &[(u64, &str)], // (chain_id, contract_address)
) -> HashMap<String, f64> {
    let coin_ids: Vec<String> = tokens
        .iter()
        .filter_map(|(chain_id, addr)| token_by_address(*chain_id, addr))
        .collect();

    if coin_ids.is_empty() {
        return HashMap::new();
    }

    let ids_param = coin_ids.join(",");
    let url = format!("{}/prices/current/{}", LLAMA_BASE, ids_param);

    let resp = match http
        .get(&url)
        .timeout(std::time::Duration::from_secs(10))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("DeFiLlama batch request failed: {}", e);
            return HashMap::new();
        }
    };

    if !resp.status().is_success() {
        warn!("DeFiLlama batch returned {}", resp.status());
        return HashMap::new();
    }

    let body: LlamaResponse = match resp.json().await {
        Ok(b) => b,
        Err(e) => {
            warn!("DeFiLlama batch parse error: {}", e);
            return HashMap::new();
        }
    };

    let mut result = HashMap::new();
    for (coin_id, coin) in body.coins {
        if let Some(addr) = coin_id.split(':').nth(1) {
            result.insert(addr.to_lowercase(), coin.price);
        }
    }
    result
}
