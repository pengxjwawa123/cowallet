use axum::{
    extract::{Query, State},
    routing::get,
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::errors::{ApiError, Result};
use crate::services::covalent;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(get_balance))
        .route("/all", get(get_all_balances))
}

#[derive(Debug, Deserialize)]
struct BalanceQuery {
    address: String,
    chain_id: Option<u64>,
}

#[derive(Debug, Serialize)]
struct BalanceResponse {
    address: String,
    chain_id: u64,
    tokens: Vec<TokenInfo>,
    total_usd: String,
}

#[derive(Debug, Serialize)]
struct TokenInfo {
    symbol: String,
    balance: String,
    usd: String,
    native: bool,
    decimals: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    logo_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    contract_address: Option<String>,
}

/// GET /balance?address={addr}&chain_id={chain_id}
/// Returns token balances from Covalent
async fn get_balance(
    State(state): State<AppState>,
    Query(query): Query<BalanceQuery>,
) -> Result<Json<BalanceResponse>> {
    // Validate address format
    if !query.address.starts_with("0x") || query.address.len() != 42 {
        return Err(ApiError::invalid_address(&query.address));
    }

    let chain_id = query.chain_id.ok_or_else(|| ApiError::bad_request("chain_id is required"))?;

    // When Covalent is not configured, use direct RPC
    if state.covalent_api_key.is_none() {
        return get_balance_via_rpc(&state, &query.address, chain_id).await;
    }

    let api_key = state.covalent_api_key.as_ref().unwrap();

    // Query Covalent for balances
    match covalent::get_balances(&state.http, api_key, &query.address, chain_id).await {
        Ok(tokens) => {
            let total_usd: f64 = tokens
                .iter()
                .filter_map(|t| t.usd.parse::<f64>().ok())
                .sum();

            let response_tokens: Vec<TokenInfo> = tokens
                .into_iter()
                .map(|t| TokenInfo {
                    symbol: t.symbol,
                    balance: t.balance_formatted,
                    usd: t.usd,
                    native: t.native_token,
                    decimals: t.decimals,
                    logo_url: t.logo_url,
                    contract_address: t.contract_address,
                })
                .collect();

            Ok(Json(BalanceResponse {
                address: query.address,
                chain_id,
                tokens: response_tokens,
                total_usd: format!("{:.2}", total_usd),
            }))
        }
        Err(_) => get_balance_via_rpc(&state, &query.address, chain_id).await,
    }
}

async fn get_balance_via_rpc(
    state: &AppState,
    address: &str,
    chain_id: u64,
) -> Result<Json<BalanceResponse>> {
    use alloy_primitives::Address;
    use std::str::FromStr;

    let owner = Address::from_str(address)
        .map_err(|_| ApiError::invalid_address(address))?;

    let rpc_url = state.rpc_for_chain(chain_id);
    let balance = chain_evm::tokens::query_native_balance(owner, rpc_url)
        .await
        .map_err(|e| ApiError::rpc_error(format!("RPC balance query failed: {}", e)))?;

    let divisor = alloy_primitives::U256::from(10).pow(alloy_primitives::U256::from(18));
    let whole = balance / divisor;
    let frac = balance % divisor;
    let formatted = if frac.is_zero() {
        format!("{}", whole)
    } else {
        let frac_str = format!("{}", frac);
        let padded = format!("{:0>18}", frac_str);
        let trimmed = padded.trim_end_matches('0');
        let display = if trimmed.len() > 6 { &trimmed[..6] } else { trimmed };
        format!("{}.{}", whole, display)
    };

    Ok(Json(BalanceResponse {
        address: address.to_string(),
        chain_id,
        tokens: vec![TokenInfo {
            symbol: "ETH".to_string(),
            balance: formatted,
            usd: "—".to_string(),
            native: true,
            decimals: 18,
            logo_url: None,
            contract_address: None,
        }],
        total_usd: "—".to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct AllBalancesQuery {
    address: String,
}

#[derive(Debug, Serialize)]
struct AllBalancesResponse {
    address: String,
    chains: Vec<ChainInfo>,
    total_usd: String,
}

#[derive(Debug, Serialize)]
struct ChainInfo {
    chain_id: u64,
    chain_name: String,
    tokens: Vec<TokenInfo>,
    total_usd: String,
}

/// GET /balance/all?address={addr}
/// Returns token balances across all supported chains from Covalent
async fn get_all_balances(
    State(state): State<AppState>,
    Query(query): Query<AllBalancesQuery>,
) -> Result<Json<AllBalancesResponse>> {
    // Validate address format
    if !query.address.starts_with("0x") || query.address.len() != 42 {
        return Err(ApiError::invalid_address(&query.address));
    }

    // Require Covalent API key for cross-chain queries
    let api_key = state.covalent_api_key.as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Covalent API not configured"))?;

    // Query all supported mainnet chains
    let chain_ids = vec![1, 8453, 42161, 10, 56, 137];

    match covalent::get_all_chain_balances(&state.http, api_key, &query.address, &chain_ids).await {
        Ok(result) => {
            let chains: Vec<ChainInfo> = result
                .chains
                .into_iter()
                .map(|chain| ChainInfo {
                    chain_id: chain.chain_id,
                    chain_name: chain.chain_name,
                    tokens: chain
                        .tokens
                        .into_iter()
                        .map(|t| TokenInfo {
                            symbol: t.symbol,
                            balance: t.balance_formatted,
                            usd: t.usd,
                            native: t.native_token,
                            decimals: t.decimals,
                            logo_url: t.logo_url,
                            contract_address: t.contract_address,
                        })
                        .collect(),
                    total_usd: chain.total_usd,
                })
                .collect();

            Ok(Json(AllBalancesResponse {
                address: result.address,
                chains,
                total_usd: result.total_usd,
            }))
        }
        Err(e) => Err(ApiError::service_unavailable(format!(
            "Failed to fetch cross-chain balances: {}",
            e
        ))),
    }
}
