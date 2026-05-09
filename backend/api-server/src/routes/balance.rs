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
    Router::new().route("/", get(get_balance))
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

    let chain_id = query.chain_id.unwrap_or(84532);

    // For testnets or when Covalent is not configured, use direct RPC
    if covalent::is_testnet(chain_id) || state.covalent_api_key.is_none() {
        return get_balance_via_rpc(&state, &query.address, chain_id).await;
    }

    let api_key = state.covalent_api_key.as_ref().unwrap();

    // Query Covalent for mainnet balances
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

    let balance = chain_evm::tokens::query_native_balance(owner, &state.rpc_url)
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
        }],
        total_usd: "—".to_string(),
    }))
}
