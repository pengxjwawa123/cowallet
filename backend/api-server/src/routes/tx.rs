use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};
use std::time::Instant;

use crate::middleware::audit::AuditResult;
use crate::middleware::auth::Claims;
use crate::retry::{is_retryable_error, retry_with_backoff, RetryConfig};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/submit", post(submit))
        .route("/status/{tx_hash}", get(tx_status))
        .route("/summary", get(spending_summary))
        .route("/simulate", post(simulate))
        .route("/estimate-gas", post(estimate_gas))
        .route("/userop", post(submit_userop))
        .route("/userop/submit", post(submit_signed_userop))
        // Merge history routes from tx_history module
        .merge(super::tx_history::router())
}

#[derive(Deserialize)]
struct SubmitRequest {
    raw_tx: String,
    chain_id: Option<u64>,
    to_addr: Option<String>,
    from_addr: Option<String>,
    value: Option<String>,
    token: Option<String>,
    mpc_session_id: Option<String>,
}

#[derive(Serialize)]
struct SubmitResponse {
    tx_hash: String,
}

async fn submit(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(body): Json<SubmitRequest>,
) -> Result<Json<SubmitResponse>, (StatusCode, Json<ErrorResponse>)> {
    let start = Instant::now();
    let user_id: uuid::Uuid = claims
        .0
        .sub
        .parse()
        .map_err(|_| rpc_error("invalid user id in token"))?;
    let chain_id = body.chain_id.ok_or_else(|| rpc_error("chain_id is required"))?;

    let rpc_url = state.rpc_for_chain(chain_id);

    tracing::info!(
        "[tx.submit] chain_id={} rpc_url={} from={:?} to={:?} value={:?} token={:?}",
        chain_id, rpc_url,
        body.from_addr, body.to_addr, body.value, body.token
    );

    // Query on-chain balance for diagnostics
    if let Some(from_addr) = &body.from_addr {
        let balance_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [from_addr, "latest"],
            "id": 1
        });
        match state.http.post(rpc_url).json(&balance_body).send().await {
            Ok(bal_resp) => {
                if let Ok(bal_json) = bal_resp.json::<serde_json::Value>().await {
                    tracing::info!("[tx.submit] eth_getBalance response: {:?}", bal_json);
                }
            }
            Err(e) => {
                tracing::warn!("[tx.submit] eth_getBalance failed: {}", e);
            }
        }
    }

    let raw_bytes = body.raw_tx.strip_prefix("0x").unwrap_or(&body.raw_tx);

    let rpc_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": [format!("0x{raw_bytes}")],
        "id": 1
    });

    // Send raw transaction to RPC (no retry for tx broadcast — retrying can cause nonce issues)
    let resp = state
        .http
        .post(rpc_url)
        .json(&rpc_body)
        .send()
        .await
        .map_err(|e| {
            tracing::error!("[tx.submit] RPC request failed: {}", e);
            rpc_error(&format!("RPC request failed: {e}"))
        })?;

    let status = resp.status();
    let rpc_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| {
            tracing::error!("[tx.submit] Failed to parse RPC response (HTTP {}): {}", status, e);
            rpc_error(&format!("Invalid RPC response (HTTP {}): {e}", status))
        })?;

    tracing::info!("[tx.submit] eth_sendRawTransaction response: {:?}", rpc_resp);

    if let Some(err) = rpc_resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown RPC error");

        tracing::error!("[tx.submit] RPC error: {} (full: {:?})", msg, err);

        // Audit log - transaction submission failed
        let _ = state
            .audit_logger
            .log_with_details(
                user_id,
                "tx.submit",
                AuditResult::Failed,
                None,
                None,
                None,
                Some(start.elapsed().as_millis() as i64),
                Some(serde_json::json!({ "error": msg, "chain_id": chain_id })),
            )
            .await;

        return Err(rpc_error(msg));
    }

    let tx_hash = rpc_resp
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .to_string();

    if let Some(db) = &state.db {
        let from_bytes = body
            .from_addr
            .as_deref()
            .and_then(|a| hex::decode(a.strip_prefix("0x").unwrap_or(a)).ok())
            .unwrap_or_default();
        let to_bytes = body
            .to_addr
            .as_deref()
            .and_then(|a| hex::decode(a.strip_prefix("0x").unwrap_or(a)).ok())
            .unwrap_or_default();
        let hash_bytes = hex::decode(tx_hash.strip_prefix("0x").unwrap_or(&tx_hash))
            .unwrap_or_default();

        if let Err(e) = sqlx::query(
            "INSERT INTO transactions (user_id, chain_id, from_addr, to_addr, value, token, tx_hash, status)
             VALUES ($1, $2, $3, $4, $5, $6, $7, 'broadcast')",
        )
        .bind(user_id)
        .bind(chain_id as i64)
        .bind(&from_bytes)
        .bind(&to_bytes)
        .bind(body.value.as_deref().unwrap_or("0"))
        .bind(&body.token)
        .bind(&hash_bytes)
        .execute(db)
        .await
        {
            tracing::warn!("failed to record transaction: {e}");
        }

        // Link transaction hash back to MPC session if session_id was provided
        if let Some(ref mpc_sid) = body.mpc_session_id {
            if let Ok(sid) = uuid::Uuid::parse_str(mpc_sid) {
                if let Err(e) = sqlx::query(
                    "UPDATE mpc_sessions SET tx_hash = $1 WHERE id = $2"
                )
                .bind(&hash_bytes)
                .bind(sid)
                .execute(db)
                .await
                {
                    tracing::warn!("failed to link tx_hash to mpc_session {}: {}", sid, e);
                } else {
                    tracing::debug!("Linked tx_hash {} to mpc_session {}", tx_hash, sid);
                }
            }
        }
    }

    // Audit log - transaction submission success
    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "tx.submit",
            AuditResult::Success,
            None,
            None,
            None,
            Some(start.elapsed().as_millis() as i64),
            Some(serde_json::json!({ "tx_hash": tx_hash, "chain_id": chain_id })),
        )
        .await;

    Ok(Json(SubmitResponse { tx_hash }))
}

// ─── Transaction Status Endpoint ────────────────────────────────────────────

#[derive(Serialize)]
struct TxStatusResponse {
    tx_hash: String,
    status: String,
    block_number: Option<i64>,
    gas_used: Option<i64>,
    confirmations: Option<i64>,
    confirmed_at: Option<String>,
}

/// GET /status/:tx_hash
///
/// Returns the current confirmation status of a transaction.
async fn tx_status(
    State(state): State<AppState>,
    _claims: axum::Extension<Claims>,
    Path(tx_hash): Path<String>,
) -> Result<Json<TxStatusResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| db_unavailable())?;

    let hash_str = tx_hash.strip_prefix("0x").unwrap_or(&tx_hash);
    let hash_bytes = hex::decode(hash_str)
        .map_err(|_| rpc_error("invalid tx_hash hex"))?;

    let row: Option<(String, Option<i64>, Option<i64>, Option<chrono::DateTime<chrono::Utc>>, i64)> =
        sqlx::query_as(
            "SELECT status, block_number, gas_used, confirmed_at, chain_id
             FROM transactions
             WHERE tx_hash = $1"
        )
        .bind(&hash_bytes)
        .fetch_optional(db)
        .await
        .map_err(|e| db_error(&e.to_string()))?;

    let (status, block_number, gas_used, confirmed_at, chain_id) = row
        .ok_or_else(|| (StatusCode::NOT_FOUND, Json(ErrorResponse { error: "transaction not found".into() })))?;

    // Calculate confirmations if confirmed
    let confirmations = if let Some(block_num) = block_number {
        // Get current block number from chain
        let rpc_url = state.rpc_for_chain(chain_id as u64);
        match get_current_block(&state.http, rpc_url).await {
            Ok(current_block) => Some(current_block - block_num),
            Err(_) => Some(0),
        }
    } else {
        None
    };

    Ok(Json(TxStatusResponse {
        tx_hash: format!("0x{}", hex::encode(&hash_bytes)),
        status,
        block_number,
        gas_used,
        confirmations,
        confirmed_at: confirmed_at.map(|t| t.to_rfc3339()),
    }))
}

/// Helper to get the current block number from an RPC endpoint.
async fn get_current_block(http: &reqwest::Client, rpc_url: &str) -> Result<i64, String> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_blockNumber",
        "params": [],
        "id": 1
    });

    let resp = http
        .post(rpc_url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("RPC request failed: {}", e))?;

    let rpc_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Invalid RPC response: {}", e))?;

    let block_hex = rpc_resp
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("0x0");

    i64::from_str_radix(
        block_hex.strip_prefix("0x").unwrap_or(block_hex),
        16,
    )
    .map_err(|e| format!("Invalid block number: {}", e))
}

#[derive(Deserialize)]
struct SummaryQuery {
    days: Option<i64>,
}

#[derive(Serialize)]
struct TokenSpend {
    token: String,
    total_value: String,
    tx_count: i64,
}

#[derive(Serialize)]
struct SpendingSummaryResponse {
    period_days: i64,
    total_transactions: i64,
    total_spend: String,
    by_token: Vec<TokenSpend>,
}

// history function removed - now handled by tx_history module

async fn spending_summary(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Query(q): Query<SummaryQuery>,
) -> Result<Json<SpendingSummaryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state
        .require_db()
        .map_err(|_| db_unavailable())?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| db_error("invalid user id in token"))?;
    let days = q.days.unwrap_or(30).max(1);

    let rows: Vec<(Option<String>, String, i64)> = sqlx::query_as(
        "SELECT token, SUM(CAST(value AS NUMERIC)) as total_value, COUNT(*) as tx_count
         FROM transactions
         WHERE user_id = $1
           AND created_at >= NOW() - ($2 || ' days')::INTERVAL
           AND status = 'confirmed'
         GROUP BY token
         ORDER BY total_value DESC",
    )
    .bind(user_id)
    .bind(days)
    .fetch_all(db)
    .await
    .map_err(|e| db_error(&e.to_string()))?;

    let mut total_transactions = 0i64;
    let mut total_spend = 0u64;
    let mut by_token = Vec::new();

    for (token, value_str, count) in rows {
        total_transactions += count;
        if let Ok(val) = value_str.parse::<u64>() {
            total_spend += val;
        }
        by_token.push(TokenSpend {
            token: token.unwrap_or_else(|| "ETH".to_string()),
            total_value: value_str,
            tx_count: count,
        });
    }

    Ok(Json(SpendingSummaryResponse {
        period_days: days,
        total_transactions,
        total_spend: total_spend.to_string(),
        by_token,
    }))
}

#[derive(Deserialize)]
struct SimulateRequest {
    to: String,
    value: Option<String>,
    data: Option<String>,
    from: Option<String>,
    chain_id: Option<u64>,
}

#[derive(Serialize)]
struct SimulateResponse {
    success: bool,
    return_data: String,
    gas_used: Option<String>,
}

async fn simulate(
    State(state): State<AppState>,
    Json(body): Json<SimulateRequest>,
) -> Result<Json<SimulateResponse>, (StatusCode, Json<ErrorResponse>)> {
    let chain_id = body.chain_id.ok_or_else(|| rpc_error("chain_id is required"))?;
    let rpc_url = state.rpc_for_chain(chain_id);

    let mut call_obj = serde_json::json!({
        "to": body.to,
    });
    if let Some(ref v) = body.value {
        call_obj["value"] = serde_json::Value::String(v.clone());
    }
    if let Some(ref d) = body.data {
        call_obj["data"] = serde_json::Value::String(d.clone());
    }
    if let Some(ref f) = body.from {
        call_obj["from"] = serde_json::Value::String(f.clone());
    }

    let rpc_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [call_obj, "latest"],
        "id": 1
    });

    let resp = state
        .http
        .post(rpc_url)
        .json(&rpc_body)
        .send()
        .await
        .map_err(|e| rpc_error(&format!("RPC request failed: {e}")))?;

    let rpc_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| rpc_error(&format!("Invalid RPC response: {e}")))?;

    if let Some(err) = rpc_resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("simulation failed");
        return Ok(Json(SimulateResponse {
            success: false,
            return_data: msg.to_string(),
            gas_used: None,
        }));
    }

    let return_data = rpc_resp
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("0x")
        .to_string();

    Ok(Json(SimulateResponse {
        success: true,
        return_data,
        gas_used: None,
    }))
}

// ─── Gas Estimation Endpoint ────────────────────────────────────────────────

#[derive(Deserialize)]
struct EstimateGasRequest {
    from: String,
    to: String,
    value: Option<String>,
    token: Option<String>,
    chain_id: Option<u64>,
}

#[derive(Serialize)]
struct EstimateGasResponse {
    gas_units: String,
    gas_price_gwei: String,
    estimated_cost_eth: String,
    estimated_cost_usd: Option<String>,
}

/// POST /estimate-gas
///
/// Estimates gas cost for a transfer by calling eth_estimateGas and eth_gasPrice
/// on the configured RPC endpoint.
async fn estimate_gas(
    State(state): State<AppState>,
    Json(body): Json<EstimateGasRequest>,
) -> Result<Json<EstimateGasResponse>, (StatusCode, Json<ErrorResponse>)> {
    let chain_id = body.chain_id.ok_or_else(|| rpc_error("chain_id is required"))?;
    let rpc_url = state.rpc_for_chain(chain_id);

    // Build the transaction object for eth_estimateGas
    let mut tx_obj = serde_json::json!({
        "from": body.from,
        "to": body.to,
    });

    // For native ETH transfer, set value; for ERC-20 tokens, we'd need contract call data
    if let Some(ref value) = body.value {
        // Value should be in wei (hex-encoded)
        let value_hex = if value.starts_with("0x") {
            value.clone()
        } else {
            // Parse as decimal wei and convert to hex
            match value.parse::<u128>() {
                Ok(v) => format!("0x{:x}", v),
                Err(_) => {
                    // Try parsing as a float ETH amount and convert to wei
                    match value.parse::<f64>() {
                        Ok(eth_amount) => {
                            let wei = (eth_amount * 1e18) as u128;
                            format!("0x{:x}", wei)
                        }
                        Err(_) => "0x0".to_string(),
                    }
                }
            }
        };
        tx_obj["value"] = serde_json::Value::String(value_hex);
    }

    // Call eth_estimateGas
    let estimate_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_estimateGas",
        "params": [tx_obj, "latest"],
        "id": 1
    });

    // Call eth_gasPrice
    let gas_price_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_gasPrice",
        "params": [],
        "id": 2
    });

    // Execute both RPC calls concurrently
    let (estimate_resp, price_resp) = tokio::join!(
        state.http.post(rpc_url).json(&estimate_body).send(),
        state.http.post(rpc_url).json(&gas_price_body).send(),
    );

    let estimate_resp = estimate_resp
        .map_err(|e| rpc_error(&format!("eth_estimateGas request failed: {e}")))?;
    let price_resp = price_resp
        .map_err(|e| rpc_error(&format!("eth_gasPrice request failed: {e}")))?;

    let estimate_json: serde_json::Value = estimate_resp
        .json()
        .await
        .map_err(|e| rpc_error(&format!("Invalid estimateGas response: {e}")))?;
    let price_json: serde_json::Value = price_resp
        .json()
        .await
        .map_err(|e| rpc_error(&format!("Invalid gasPrice response: {e}")))?;

    // Parse gas units from hex
    let gas_hex = estimate_json
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("0x5208"); // default 21000 for simple transfer
    let gas_units = u64::from_str_radix(gas_hex.strip_prefix("0x").unwrap_or(gas_hex), 16)
        .unwrap_or(21000);

    // Parse gas price from hex (in wei)
    let price_hex = price_json
        .get("result")
        .and_then(|r| r.as_str())
        .unwrap_or("0x0");
    let gas_price_wei = u128::from_str_radix(price_hex.strip_prefix("0x").unwrap_or(price_hex), 16)
        .unwrap_or(0);

    // Calculate estimated cost
    let gas_price_gwei = gas_price_wei as f64 / 1e9;
    let estimated_cost_wei = gas_units as u128 * gas_price_wei;
    let estimated_cost_eth = estimated_cost_wei as f64 / 1e18;

    // Try to get ETH price for USD estimate
    let estimated_cost_usd = state
        .price_cache
        .get_usd_price(&state.http, "ETH")
        .await
        .map(|eth_price| format!("${:.2}", estimated_cost_eth * eth_price));

    Ok(Json(EstimateGasResponse {
        gas_units: gas_units.to_string(),
        gas_price_gwei: format!("{:.2}", gas_price_gwei),
        estimated_cost_eth: format!("{:.6}", estimated_cost_eth),
        estimated_cost_usd,
    }))
}

// ─── ERC-4337 UserOperation Endpoints ───────────────────────────────────────

/// Default ERC-4337 v0.6 EntryPoint address
const DEFAULT_ENTRY_POINT: &str = "0x5FF137D4b0FDCD49DcA30c7CF57E578a026d2789";

#[derive(Deserialize)]
struct SubmitUserOpRequest {
    sender: String,
    nonce: String,
    call_data: String,
    chain_id: Option<u64>,
    #[allow(dead_code)]
    bundler_url: Option<String>,
}

#[derive(Serialize)]
struct SubmitUserOpResponse {
    session_id: String,
    user_op_hash: String,
}

/// POST /userop
///
/// Constructs a UserOperation, computes the userOpHash, and initiates
/// an MPC signing session. The actual signature is completed asynchronously.
async fn submit_userop(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(body): Json<SubmitUserOpRequest>,
) -> Result<Json<SubmitUserOpResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| db_unavailable())?;
    let user_id: uuid::Uuid = claims
        .0
        .sub
        .parse()
        .map_err(|_| rpc_error("invalid user id in token"))?;

    let chain_id = body.chain_id.ok_or_else(|| rpc_error("chain_id is required"))?;

    // Parse sender address
    let sender_str = body.sender.strip_prefix("0x").unwrap_or(&body.sender);
    let sender_bytes = hex::decode(sender_str)
        .map_err(|_| rpc_error("invalid sender address"))?;
    if sender_bytes.len() != 20 {
        return Err(rpc_error("sender address must be 20 bytes"));
    }
    let sender = alloy_primitives::Address::from_slice(&sender_bytes);

    // Parse nonce
    let nonce_str = body.nonce.strip_prefix("0x").unwrap_or(&body.nonce);
    let nonce = alloy_primitives::U256::from_str_radix(nonce_str, 16)
        .map_err(|_| rpc_error("invalid nonce hex value"))?;

    // Parse call_data
    let call_data_str = body.call_data.strip_prefix("0x").unwrap_or(&body.call_data);
    let call_data_bytes = hex::decode(call_data_str)
        .map_err(|_| rpc_error("invalid call_data hex"))?;

    // Build UserOperation
    let user_op = chain_evm::userop::UserOperation::new(
        sender,
        nonce,
        alloy_primitives::Bytes::from(call_data_bytes),
    );

    // Parse entry point
    let ep_str = DEFAULT_ENTRY_POINT.strip_prefix("0x").unwrap_or(DEFAULT_ENTRY_POINT);
    let ep_bytes = hex::decode(ep_str).expect("default entry point is valid hex");
    let entry_point = alloy_primitives::Address::from_slice(&ep_bytes);

    // Compute userOpHash
    let user_op_hash = user_op.hash(entry_point, chain_id);
    let user_op_hash_hex = format!("0x{}", hex::encode(user_op_hash.as_slice()));

    // Create an MPC signing session with the hash
    let session_id = uuid::Uuid::new_v4();
    let parties: Vec<i16> = vec![0, 1]; // device + server

    sqlx::query(
        "INSERT INTO mpc_sessions (id, user_id, session_type, parties, threshold, status, current_round)
         VALUES ($1, $2, 'sign', $3, 2, 'active', 0)"
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&parties)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create MPC signing session for userop: {}", e);
        rpc_error("failed to create signing session")
    })?;

    // Notify the server MPC participant
    if let Some(participant) = &state.mpc_participant {
        if let Err(e) = participant
            .on_session_created(session_id, user_id, "sign", &parties, 2, None)
            .await
        {
            tracing::error!(
                "Server participant failed to join userop session {}: {}",
                session_id,
                e
            );
        }
    }

    tracing::info!(
        "Created userop signing session {} for user {}, hash={}",
        session_id,
        user_id,
        user_op_hash_hex
    );

    Ok(Json(SubmitUserOpResponse {
        session_id: session_id.to_string(),
        user_op_hash: user_op_hash_hex,
    }))
}

#[derive(Deserialize)]
struct SubmitSignedUserOpRequest {
    user_op: serde_json::Value,
    chain_id: Option<u64>,
    bundler_url: Option<String>,
}

#[derive(Serialize)]
struct SubmitSignedUserOpResponse {
    user_op_hash: String,
}

/// POST /userop/submit
///
/// Submits an already-signed UserOperation to a bundler via eth_sendUserOperation.
async fn submit_signed_userop(
    State(state): State<AppState>,
    _claims: axum::Extension<Claims>,
    Json(body): Json<SubmitSignedUserOpRequest>,
) -> Result<Json<SubmitSignedUserOpResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _chain_id = body.chain_id.ok_or_else(|| rpc_error("chain_id is required"))?;

    // Parse the UserOperation from the JSON value
    let op = &body.user_op;

    let parse_address = |field: &str| -> Result<alloy_primitives::Address, (StatusCode, Json<ErrorResponse>)> {
        let val = op.get(field).and_then(|v| v.as_str())
            .ok_or_else(|| rpc_error(&format!("missing field: {}", field)))?;
        let s = val.strip_prefix("0x").unwrap_or(val);
        let bytes = hex::decode(s).map_err(|_| rpc_error(&format!("invalid hex for {}", field)))?;
        if bytes.len() != 20 {
            return Err(rpc_error(&format!("{} must be 20 bytes", field)));
        }
        Ok(alloy_primitives::Address::from_slice(&bytes))
    };

    let parse_u256 = |field: &str| -> Result<alloy_primitives::U256, (StatusCode, Json<ErrorResponse>)> {
        let val = op.get(field).and_then(|v| v.as_str())
            .ok_or_else(|| rpc_error(&format!("missing field: {}", field)))?;
        let s = val.strip_prefix("0x").unwrap_or(val);
        alloy_primitives::U256::from_str_radix(s, 16)
            .map_err(|_| rpc_error(&format!("invalid hex U256 for {}", field)))
    };

    let parse_bytes = |field: &str| -> Result<alloy_primitives::Bytes, (StatusCode, Json<ErrorResponse>)> {
        let val = op.get(field).and_then(|v| v.as_str()).unwrap_or("0x");
        let s = val.strip_prefix("0x").unwrap_or(val);
        let bytes = hex::decode(s).map_err(|_| rpc_error(&format!("invalid hex for {}", field)))?;
        Ok(alloy_primitives::Bytes::from(bytes))
    };

    let sender = parse_address("sender")?;
    let nonce = parse_u256("nonce")?;
    let init_code = parse_bytes("initCode")?;
    let call_data = parse_bytes("callData")?;
    let call_gas_limit = parse_u256("callGasLimit")?;
    let verification_gas_limit = parse_u256("verificationGasLimit")?;
    let pre_verification_gas = parse_u256("preVerificationGas")?;
    let max_fee_per_gas = parse_u256("maxFeePerGas")?;
    let max_priority_fee_per_gas = parse_u256("maxPriorityFeePerGas")?;
    let paymaster_and_data = parse_bytes("paymasterAndData")?;
    let signature = parse_bytes("signature")?;

    let user_op = chain_evm::userop::UserOperation {
        sender,
        nonce,
        init_code,
        call_data,
        call_gas_limit,
        verification_gas_limit,
        pre_verification_gas,
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data,
        signature,
    };

    // Determine bundler URL
    let bundler_url = body
        .bundler_url
        .or_else(|| std::env::var("BUNDLER_URL").ok())
        .ok_or_else(|| rpc_error("no bundler URL configured"))?;

    // Parse entry point
    let ep_str = DEFAULT_ENTRY_POINT.strip_prefix("0x").unwrap_or(DEFAULT_ENTRY_POINT);
    let ep_bytes = hex::decode(ep_str).expect("default entry point is valid hex");
    let entry_point = alloy_primitives::Address::from_slice(&ep_bytes);

    // Submit to bundler
    let op_hash = user_op
        .submit_to_bundler(&state.http, &bundler_url, entry_point)
        .await
        .map_err(|e| rpc_error(&e))?;

    Ok(Json(SubmitSignedUserOpResponse {
        user_op_hash: format!("0x{}", hex::encode(op_hash.as_slice())),
    }))
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn rpc_error(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_GATEWAY,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}

fn db_unavailable() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ErrorResponse {
            error: "database not available".into(),
        }),
    )
}

fn db_error(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}
