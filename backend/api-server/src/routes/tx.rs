use axum::{
    Json, Router,
    extract::{Query, State},
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
        .route("/history", get(history))
        .route("/summary", get(spending_summary))
        .route("/simulate", post(simulate))
}

#[derive(Deserialize)]
struct SubmitRequest {
    raw_tx: String,
    chain_id: Option<u64>,
    to_addr: Option<String>,
    value: Option<String>,
    token: Option<String>,
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
    let chain_id = body.chain_id.unwrap_or(84532);

    let rpc_url = &state.rpc_url;
    let raw_bytes = body.raw_tx.strip_prefix("0x").unwrap_or(&body.raw_tx);

    let rpc_body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_sendRawTransaction",
        "params": [format!("0x{raw_bytes}")],
        "id": 1
    });

    // Use circuit breaker and retry for RPC calls
    let resp_result = state
        .rpc_circuit_breaker
        .call(|| async {
            retry_with_backoff(
                RetryConfig::default(),
                || async {
                    state
                        .http
                        .post(rpc_url)
                        .json(&rpc_body)
                        .send()
                        .await
                        .and_then(|r| r.error_for_status())
                },
                is_retryable_error,
                "eth_sendRawTransaction",
            )
            .await
        })
        .await;

    let resp = match resp_result {
        Ok(r) => r,
        Err(None) => {
            return Err(rpc_error(
                "RPC service unavailable - circuit breaker open",
            ));
        }
        Err(Some(e)) => {
            return Err(rpc_error(&format!("RPC request failed: {e}")));
        }
    };

    let rpc_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| rpc_error(&format!("Invalid RPC response: {e}")))?;

    if let Some(err) = rpc_resp.get("error") {
        let msg = err
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown RPC error");

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
        .bind(&[] as &[u8])
        .bind(&to_bytes)
        .bind(body.value.as_deref().unwrap_or("0"))
        .bind(&body.token)
        .bind(&hash_bytes)
        .execute(db)
        .await
        {
            tracing::warn!("failed to record transaction: {e}");
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

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct SummaryQuery {
    days: Option<i64>,
}

#[derive(Serialize)]
struct TxRecord {
    id: String,
    chain_id: i64,
    to_addr: String,
    value: String,
    token: Option<String>,
    tx_hash: Option<String>,
    status: String,
    created_at: String,
}

#[derive(Serialize)]
struct HistoryResponse {
    transactions: Vec<TxRecord>,
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

async fn history(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state
        .require_db()
        .map_err(|_| db_unavailable())?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| db_error("invalid user id in token"))?;
    let limit = q.limit.unwrap_or(20).min(100);
    let offset = q.offset.unwrap_or(0);

    let rows: Vec<(uuid::Uuid, i64, Vec<u8>, String, Option<String>, Option<Vec<u8>>, String, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, chain_id, to_addr, value, token, tx_hash, status, created_at
             FROM transactions WHERE user_id = $1
             ORDER BY created_at DESC LIMIT $2 OFFSET $3",
        )
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(db)
        .await
        .map_err(|e| db_error(&e.to_string()))?;

    let transactions = rows
        .into_iter()
        .map(|(id, chain_id, to_addr, value, token, tx_hash, status, created_at)| {
            TxRecord {
                id: id.to_string(),
                chain_id,
                to_addr: format!("0x{}", hex::encode(&to_addr)),
                value,
                token,
                tx_hash: tx_hash.map(|h| format!("0x{}", hex::encode(&h))),
                status,
                created_at: created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(HistoryResponse { transactions }))
}

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
    let rpc_url = &state.rpc_url;

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
