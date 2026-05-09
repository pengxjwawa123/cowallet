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

    // Check if Covalent API key is configured
    let api_key = state
        .covalent_api_key
        .as_ref()
        .ok_or_else(|| ApiError::service_unavailable("Balance service not configured"))?;

    let chain_id = query.chain_id.unwrap_or(84532); // Default to Base Sepolia

    // Query Covalent for balances
    let tokens = covalent::get_balances(&state.http, api_key, &query.address, chain_id)
        .await
        .map_err(|e| ApiError::rpc_error(format!("Balance query failed: {}", e)))?;

    // Calculate total USD value
    let total_usd: f64 = tokens
        .iter()
        .filter_map(|t| t.usd.parse::<f64>().ok())
        .sum();

    // Convert to response format
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
