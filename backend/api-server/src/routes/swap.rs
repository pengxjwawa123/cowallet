//! Swap/DEX routes for token exchange via 0x aggregator.
//!
//! GET  /api/v1/swap/quote  — Get swap price quote
//! POST /api/v1/swap/build  — Build swap transaction calldata

use axum::{
    extract::{Query, State},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::errors::{ApiError, Result};
use crate::services::dex;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/quote", get(get_swap_quote))
        .route("/build", post(build_swap_tx))
}

// ─── Request / Response types ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct QuoteQuery {
    chain_id: u64,
    sell_token: String,
    buy_token: String,
    sell_amount: String,
}

#[derive(Debug, Serialize)]
struct QuoteResponse {
    sell_token: String,
    buy_token: String,
    sell_amount: String,
    buy_amount: String,
    buy_amount_formatted: String,
    price: String,
    price_impact: Option<String>,
    estimated_gas: String,
    sources: Vec<String>,
    chain_id: u64,
}

#[derive(Debug, Deserialize)]
struct BuildRequest {
    chain_id: u64,
    sell_token: String,
    buy_token: String,
    sell_amount: String,
    slippage: Option<f64>,
    taker_address: String,
}

#[derive(Debug, Serialize)]
struct BuildResponse {
    to: String,
    data: String,
    value: String,
    gas_estimate: String,
    sell_token: String,
    buy_token: String,
    sell_amount: String,
    buy_amount: String,
    price: String,
    allowance_target: Option<String>,
    chain_id: u64,
}

// ─── Handlers ───────────────────────────────────────────────────────────────

/// GET /swap/quote?chain_id=8453&sell_token=ETH&buy_token=USDC&sell_amount=0.1
async fn get_swap_quote(
    State(state): State<AppState>,
    Query(query): Query<QuoteQuery>,
) -> Result<Json<QuoteResponse>> {
    // Resolve token symbols to addresses
    let sell_addr = resolve_token_address(&query.sell_token, query.chain_id)?;
    let buy_addr = resolve_token_address(&query.buy_token, query.chain_id)?;

    // Convert human-readable amount to raw
    let sell_decimals = dex::token_decimals(&query.sell_token);
    let raw_amount = dex::amount_to_raw(&query.sell_amount, sell_decimals)
        .map_err(|e| ApiError::bad_request(&e))?;

    let quote = dex::get_quote(
        &state.http,
        state.zerox_api_key.as_deref(),
        query.chain_id,
        &sell_addr,
        &buy_addr,
        &raw_amount,
    )
    .await
    .map_err(|e| ApiError::external_service(&e))?;

    let buy_decimals = dex::token_decimals(&query.buy_token);
    let buy_formatted = dex::raw_to_amount(&quote.buy_amount, buy_decimals);

    Ok(Json(QuoteResponse {
        sell_token: query.sell_token,
        buy_token: query.buy_token,
        sell_amount: query.sell_amount,
        buy_amount: quote.buy_amount,
        buy_amount_formatted: buy_formatted,
        price: quote.price,
        price_impact: quote.price_impact,
        estimated_gas: quote.estimated_gas,
        sources: quote.sources,
        chain_id: query.chain_id,
    }))
}

/// POST /swap/build — Build swap transaction with calldata
async fn build_swap_tx(
    State(state): State<AppState>,
    Json(req): Json<BuildRequest>,
) -> Result<Json<BuildResponse>> {
    // Resolve token symbols to addresses
    let sell_addr = resolve_token_address(&req.sell_token, req.chain_id)?;
    let buy_addr = resolve_token_address(&req.buy_token, req.chain_id)?;

    // Convert human-readable amount to raw
    let sell_decimals = dex::token_decimals(&req.sell_token);
    let raw_amount = dex::amount_to_raw(&req.sell_amount, sell_decimals)
        .map_err(|e| ApiError::bad_request(&e))?;

    let slippage = req.slippage.unwrap_or(0.5);

    // Validate taker address
    if !req.taker_address.starts_with("0x") || req.taker_address.len() != 42 {
        return Err(ApiError::bad_request("Invalid taker_address format"));
    }

    let tx = dex::build_swap_tx(
        &state.http,
        state.zerox_api_key.as_deref(),
        req.chain_id,
        &sell_addr,
        &buy_addr,
        &raw_amount,
        slippage,
        &req.taker_address,
    )
    .await
    .map_err(|e| ApiError::external_service(&e))?;

    Ok(Json(BuildResponse {
        to: tx.to,
        data: tx.data,
        value: tx.value,
        gas_estimate: tx.gas_estimate,
        sell_token: req.sell_token,
        buy_token: req.buy_token,
        sell_amount: req.sell_amount,
        buy_amount: tx.buy_amount,
        price: tx.price,
        allowance_target: tx.allowance_target,
        chain_id: req.chain_id,
    }))
}

// ─── Helpers ────────────────────────────────────────────────────────────────

/// Resolve a token symbol or address to a valid contract address for the 0x API
fn resolve_token_address(token: &str, chain_id: u64) -> Result<String> {
    // If already an address, pass through
    if token.starts_with("0x") && token.len() == 42 {
        return Ok(token.to_string());
    }

    // Resolve known symbols
    dex::token_address(token, chain_id)
        .map(|s| s.to_string())
        .ok_or_else(|| ApiError::bad_request(&format!(
            "Unknown token '{}' on chain {}. Provide a contract address instead.",
            token, chain_id
        )))
}
