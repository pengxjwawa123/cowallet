use axum::{
    Json, Router,
    extract::{Query, Path, State},
    http::StatusCode,
    routing::get,
};
use serde::{Deserialize, Serialize};

use crate::middleware::auth::Claims;
use crate::services::covalent;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/history", get(get_history))
        .route("/tx-history", get(get_covalent_history))
        .route("/{hash}", get(get_transaction))
}

#[derive(Deserialize)]
struct HistoryQuery {
    address: String,
    chain_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Serialize)]
struct TransactionRecord {
    tx_hash: String,
    from: String,
    to: String,
    value: String,
    token_address: Option<String>,
    status: String,
    block_number: Option<i64>,
    timestamp: Option<String>,
    chain_id: i64,
}

#[derive(Serialize)]
struct HistoryResponse {
    transactions: Vec<TransactionRecord>,
    total: i64,
}

/// GET /api/v1/tx/history?address={addr}&chain_id={id}&limit=50&offset=0
///
/// Get paginated transaction history for an address
async fn get_history(
    State(state): State<AppState>,
    _claims: axum::Extension<Claims>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<HistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| db_unavailable())?;

    // Parse address
    let address_str = q.address.strip_prefix("0x").unwrap_or(&q.address);
    let address_bytes = hex::decode(address_str)
        .map_err(|_| validation_error("invalid address hex"))?;

    if address_bytes.len() != 20 {
        return Err(validation_error("address must be 20 bytes"));
    }

    let limit = q.limit.unwrap_or(50).min(100).max(1);
    let offset = q.offset.unwrap_or(0).max(0);

    // Build query with optional chain_id filter
    let query = if let Some(chain_id) = q.chain_id {
        sqlx::query_as::<_, (Vec<u8>, Vec<u8>, Vec<u8>, String, Option<Vec<u8>>, String, Option<i64>, Option<chrono::DateTime<chrono::Utc>>, i64)>(
            r#"
            SELECT tx_hash, from_addr, to_addr, value, token_address, status, block_number, created_at, chain_id
            FROM transactions
            WHERE (from_addr = $1 OR to_addr = $1)
              AND chain_id = $2
            ORDER BY block_number DESC NULLS LAST, created_at DESC
            LIMIT $3 OFFSET $4
            "#
        )
        .bind(&address_bytes)
        .bind(chain_id)
        .bind(limit)
        .bind(offset)
    } else {
        sqlx::query_as::<_, (Vec<u8>, Vec<u8>, Vec<u8>, String, Option<Vec<u8>>, String, Option<i64>, Option<chrono::DateTime<chrono::Utc>>, i64)>(
            r#"
            SELECT tx_hash, from_addr, to_addr, value, token_address, status, block_number, created_at, chain_id
            FROM transactions
            WHERE from_addr = $1 OR to_addr = $1
            ORDER BY block_number DESC NULLS LAST, created_at DESC
            LIMIT $2 OFFSET $3
            "#
        )
        .bind(&address_bytes)
        .bind(limit)
        .bind(offset)
    };

    let rows = query.fetch_all(db).await
        .map_err(|e| db_error(&e.to_string()))?;

    // Get total count for pagination
    let total_query = if let Some(chain_id) = q.chain_id {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM transactions WHERE (from_addr = $1 OR to_addr = $1) AND chain_id = $2"
        )
        .bind(&address_bytes)
        .bind(chain_id)
    } else {
        sqlx::query_as::<_, (i64,)>(
            "SELECT COUNT(*) FROM transactions WHERE from_addr = $1 OR to_addr = $1"
        )
        .bind(&address_bytes)
    };

    let total = total_query.fetch_one(db).await
        .map(|(count,)| count)
        .unwrap_or(0);

    let transactions = rows
        .into_iter()
        .map(|(tx_hash, from_addr, to_addr, value, token_address, status, block_number, created_at, chain_id)| {
            TransactionRecord {
                tx_hash: format!("0x{}", hex::encode(&tx_hash)),
                from: format!("0x{}", hex::encode(&from_addr)),
                to: format!("0x{}", hex::encode(&to_addr)),
                value,
                token_address: token_address.map(|addr| {
                    // Zero address means native ETH
                    if addr.iter().all(|&b| b == 0) {
                        "native".to_string()
                    } else {
                        format!("0x{}", hex::encode(&addr))
                    }
                }),
                status,
                block_number,
                timestamp: created_at.map(|t| t.to_rfc3339()),
                chain_id,
            }
        })
        .collect();

    Ok(Json(HistoryResponse {
        transactions,
        total,
    }))
}

#[derive(Serialize)]
struct TransactionDetail {
    tx_hash: String,
    from: String,
    to: String,
    value: String,
    token_address: Option<String>,
    status: String,
    block_number: Option<i64>,
    timestamp: Option<String>,
    chain_id: i64,
    gas_used: Option<i64>,
}

/// GET /api/v1/tx/{hash}
///
/// Get single transaction details by hash
async fn get_transaction(
    State(state): State<AppState>,
    _claims: axum::Extension<Claims>,
    Path(hash): Path<String>,
) -> Result<Json<TransactionDetail>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| db_unavailable())?;

    // Parse transaction hash
    let hash_str = hash.strip_prefix("0x").unwrap_or(&hash);
    let hash_bytes = hex::decode(hash_str)
        .map_err(|_| validation_error("invalid transaction hash"))?;

    let row: (Vec<u8>, Vec<u8>, Vec<u8>, String, Option<Vec<u8>>, String, Option<i64>, Option<chrono::DateTime<chrono::Utc>>, i64, Option<i64>) =
        sqlx::query_as(
            r#"
            SELECT tx_hash, from_addr, to_addr, value, token_address, status, block_number, created_at, chain_id, gas_used
            FROM transactions
            WHERE tx_hash = $1
            LIMIT 1
            "#
        )
        .bind(&hash_bytes)
        .fetch_optional(db)
        .await
        .map_err(|e| db_error(&e.to_string()))?
        .ok_or_else(|| not_found("transaction not found"))?;

    let (tx_hash, from_addr, to_addr, value, token_address, status, block_number, created_at, chain_id, gas_used) = row;

    Ok(Json(TransactionDetail {
        tx_hash: format!("0x{}", hex::encode(&tx_hash)),
        from: format!("0x{}", hex::encode(&from_addr)),
        to: format!("0x{}", hex::encode(&to_addr)),
        value,
        token_address: token_address.map(|addr| {
            if addr.iter().all(|&b| b == 0) {
                "native".to_string()
            } else {
                format!("0x{}", hex::encode(&addr))
            }
        }),
        status,
        block_number,
        timestamp: created_at.map(|t| t.to_rfc3339()),
        chain_id,
        gas_used,
    }))
}

// ─── Covalent-based on-chain tx history ──────────────────────────────────────

#[derive(Deserialize)]
struct CovalentHistoryQuery {
    address: String,
    chain_id: Option<u64>,
}

#[derive(Serialize)]
struct CovalentHistoryResponse {
    transactions: Vec<CovalentTxInfo>,
    total: usize,
}

#[derive(Serialize)]
struct CovalentTxInfo {
    tx_hash: String,
    from: String,
    to: String,
    value: String,
    timestamp: String,
    status: String,
    gas_used: u64,
    token_symbol: String,
    value_quote: f64,
}

/// GET /api/v1/tx/tx-history?address={addr}&chain_id={id}
///
/// Get on-chain transaction history via Covalent API (no DB required)
async fn get_covalent_history(
    State(state): State<AppState>,
    _claims: axum::Extension<Claims>,
    Query(q): Query<CovalentHistoryQuery>,
) -> Result<Json<CovalentHistoryResponse>, (StatusCode, Json<ErrorResponse>)> {
    // Validate address
    if !q.address.starts_with("0x") || q.address.len() != 42 {
        return Err(validation_error("invalid address format"));
    }

    let chain_id = q.chain_id.unwrap_or(84532);

    // Testnets not supported by Covalent — return empty
    if covalent::is_testnet(chain_id) {
        return Ok(Json(CovalentHistoryResponse {
            transactions: vec![],
            total: 0,
        }));
    }

    let api_key = state
        .covalent_api_key
        .as_ref()
        .ok_or_else(|| (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "Covalent API not configured".into(),
            }),
        ))?;

    let items = covalent::get_transactions(&state.http, api_key, &q.address, chain_id)
        .await
        .map_err(|e| (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("Transaction history query failed: {}", e),
            }),
        ))?;

    let total = items.len();
    let transactions: Vec<CovalentTxInfo> = items
        .into_iter()
        .map(|item| CovalentTxInfo {
            tx_hash: item.tx_hash,
            from: item.from,
            to: item.to,
            value: item.value,
            timestamp: item.timestamp,
            status: item.status,
            gas_used: item.gas_used,
            token_symbol: item.token_symbol,
            value_quote: item.value_quote,
        })
        .collect();

    Ok(Json(CovalentHistoryResponse {
        transactions,
        total,
    }))
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
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

fn validation_error(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}

fn not_found(msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: msg.to_string(),
        }),
    )
}
