use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post, put},
};
use policy_engine::{Decision, Policy, PolicyAction, Rule, TransactionHistory};
use serde::{Deserialize, Serialize};

use crate::middleware::auth::Claims;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_policies))
        .route("/", post(create_policy))
        .route("/{id}", get(get_policy))
        .route("/{id}", put(update_policy))
        .route("/{id}", delete(delete_policy))
        .route("/evaluate", post(evaluate_transaction))
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.to_string() }))
}

#[derive(Serialize)]
struct PolicyResponse {
    id: String,
    name: String,
    description: String,
    rules: serde_json::Value,
    action: serde_json::Value,
    enabled: bool,
    priority: i32,
    created_at: String,
}

#[derive(Serialize)]
struct ListResponse {
    policies: Vec<PolicyResponse>,
}

async fn list_policies(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
) -> Result<Json<ListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;

    let rows: Vec<(uuid::Uuid, String, String, serde_json::Value, serde_json::Value, bool, i32, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT id, name, description, rules, action, enabled, priority, created_at
             FROM policies WHERE user_id = $1 ORDER BY priority DESC",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let policies = rows
        .into_iter()
        .map(|(id, name, description, rules, action, enabled, priority, created_at)| PolicyResponse {
            id: id.to_string(),
            name,
            description,
            rules,
            action,
            enabled,
            priority,
            created_at: created_at.to_rfc3339(),
        })
        .collect();

    Ok(Json(ListResponse { policies }))
}

#[derive(Deserialize)]
struct CreateRequest {
    name: String,
    description: Option<String>,
    rules: serde_json::Value,
    action: serde_json::Value,
    enabled: Option<bool>,
    priority: Option<i32>,
}

async fn create_policy(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(body): Json<CreateRequest>,
) -> Result<(StatusCode, Json<PolicyResponse>), (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;
    let id = uuid::Uuid::new_v4();

    let description = body.description.unwrap_or_default();
    let enabled = body.enabled.unwrap_or(true);
    let priority = body.priority.unwrap_or(0);

    sqlx::query(
        "INSERT INTO policies (id, user_id, name, description, rules, action, enabled, priority)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id)
    .bind(user_id)
    .bind(&body.name)
    .bind(&description)
    .bind(&body.rules)
    .bind(&body.action)
    .bind(enabled)
    .bind(priority)
    .execute(db)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok((
        StatusCode::CREATED,
        Json(PolicyResponse {
            id: id.to_string(),
            name: body.name,
            description,
            rules: body.rules,
            action: body.action,
            enabled,
            priority,
            created_at: chrono::Utc::now().to_rfc3339(),
        }),
    ))
}

async fn get_policy(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<PolicyResponse>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;
    let policy_id: uuid::Uuid = id.parse().map_err(|_| err(StatusCode::BAD_REQUEST, "invalid policy id"))?;

    let row: (uuid::Uuid, String, String, serde_json::Value, serde_json::Value, bool, i32, chrono::DateTime<chrono::Utc>) =
        sqlx::query_as(
            "SELECT id, name, description, rules, action, enabled, priority, created_at
             FROM policies WHERE id = $1 AND user_id = $2",
        )
        .bind(policy_id)
        .bind(user_id)
        .fetch_one(db)
        .await
        .map_err(|_| err(StatusCode::NOT_FOUND, "policy not found"))?;

    Ok(Json(PolicyResponse {
        id: row.0.to_string(),
        name: row.1,
        description: row.2,
        rules: row.3,
        action: row.4,
        enabled: row.5,
        priority: row.6,
        created_at: row.7.to_rfc3339(),
    }))
}

#[derive(Deserialize)]
struct UpdateRequest {
    name: Option<String>,
    description: Option<String>,
    rules: Option<serde_json::Value>,
    action: Option<serde_json::Value>,
    enabled: Option<bool>,
    priority: Option<i32>,
}

async fn update_policy(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Path(id): Path<String>,
    Json(body): Json<UpdateRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;
    let policy_id: uuid::Uuid = id.parse().map_err(|_| err(StatusCode::BAD_REQUEST, "invalid policy id"))?;

    let mut set_clauses = Vec::new();
    let mut param_idx = 3u32; // $1 = policy_id, $2 = user_id

    if body.name.is_some() {
        set_clauses.push(format!("name = ${param_idx}"));
        param_idx += 1;
    }
    if body.description.is_some() {
        set_clauses.push(format!("description = ${param_idx}"));
        param_idx += 1;
    }
    if body.rules.is_some() {
        set_clauses.push(format!("rules = ${param_idx}"));
        param_idx += 1;
    }
    if body.action.is_some() {
        set_clauses.push(format!("action = ${param_idx}"));
        param_idx += 1;
    }
    if body.enabled.is_some() {
        set_clauses.push(format!("enabled = ${param_idx}"));
        param_idx += 1;
    }
    if body.priority.is_some() {
        set_clauses.push(format!("priority = ${param_idx}"));
        param_idx += 1;
    }

    if set_clauses.is_empty() {
        return Ok(Json(serde_json::json!({ "updated": false, "reason": "no fields to update" })));
    }

    let _ = param_idx;
    set_clauses.push("updated_at = NOW()".to_string());
    let sql = format!(
        "UPDATE policies SET {} WHERE id = $1 AND user_id = $2",
        set_clauses.join(", ")
    );

    let mut query = sqlx::query(&sql)
        .bind(policy_id)
        .bind(user_id);

    if let Some(ref name) = body.name { query = query.bind(name); }
    if let Some(ref desc) = body.description { query = query.bind(desc); }
    if let Some(ref rules) = body.rules { query = query.bind(rules); }
    if let Some(ref action) = body.action { query = query.bind(action); }
    if let Some(enabled) = body.enabled { query = query.bind(enabled); }
    if let Some(priority) = body.priority { query = query.bind(priority); }

    let result = query
        .execute(db)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(err(StatusCode::NOT_FOUND, "policy not found"));
    }

    Ok(Json(serde_json::json!({ "updated": true })))
}

async fn delete_policy(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;
    let policy_id: uuid::Uuid = id.parse().map_err(|_| err(StatusCode::BAD_REQUEST, "invalid policy id"))?;

    let result = sqlx::query("DELETE FROM policies WHERE id = $1 AND user_id = $2")
        .bind(policy_id)
        .bind(user_id)
        .execute(db)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    if result.rows_affected() == 0 {
        return Err(err(StatusCode::NOT_FOUND, "policy not found"));
    }

    Ok(Json(serde_json::json!({ "deleted": true })))
}

#[derive(Deserialize)]
struct EvaluateRequest {
    from: String,
    to: String,
    value: String,
    token: Option<String>,
    chain_id: Option<u64>,
    is_contract: Option<bool>,
}

async fn evaluate_transaction(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(body): Json<EvaluateRequest>,
) -> Result<Json<Decision>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;

    let rows: Vec<(serde_json::Value, serde_json::Value, String, bool, i32, uuid::Uuid)> =
        sqlx::query_as(
            "SELECT rules, action, name, enabled, priority, id
             FROM policies WHERE user_id = $1 AND enabled = true
             ORDER BY priority DESC",
        )
        .bind(user_id)
        .fetch_all(db)
        .await
        .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let policies: Vec<Policy> = rows
        .into_iter()
        .filter_map(|(rules_json, action_json, name, enabled, priority, id)| {
            let rules: Vec<Rule> = serde_json::from_value(rules_json).ok()?;
            let action: PolicyAction = serde_json::from_value(action_json).ok()?;
            Some(Policy {
                id,
                name,
                description: String::new(),
                rules,
                action,
                enabled,
                priority: priority as u32,
            })
        })
        .collect();

    let to_addr: alloy_primitives::Address = body
        .to
        .parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid to address"))?;

    let from_addr: alloy_primitives::Address = body
        .from
        .parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid from address"))?;

    let value = alloy_primitives::U256::from_str_radix(
        body.value.strip_prefix("0x").unwrap_or(&body.value),
        if body.value.starts_with("0x") { 16 } else { 10 },
    )
    .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid value"))?;

    let chain_id = body.chain_id.unwrap_or(8453);

    // Compute transaction history for daily-limit / rate-limit rules
    let history = compute_tx_history(&state, &body.from, chain_id).await;

    let tx_ctx = policy_engine::types::TransactionContext {
        user_id: user_id.to_string(),
        from: from_addr,
        to: to_addr,
        value,
        token: body.token,
        chain_id,
        is_contract_interaction: body.is_contract.unwrap_or(false),
        timestamp: chrono::Utc::now(),
        history,
    };

    let decision = policy_engine::rules::evaluate(&tx_ctx, &policies);
    Ok(Json(decision))
}

/// Fetch recent transaction history from Covalent and compute aggregates
/// for policy evaluation (daily total, tx count in window).
async fn compute_tx_history(
    state: &AppState,
    from_address: &str,
    chain_id: u64,
) -> Option<TransactionHistory> {
    use crate::services::covalent;

    let api_key = state.covalent_api_key.as_ref()?;
    let txs = covalent::get_transactions(&state.http, api_key, from_address, chain_id)
        .await
        .ok()?;

    let now = chrono::Utc::now();
    let day_ago = now - chrono::Duration::hours(24);
    let hour_ago = now - chrono::Duration::hours(1);

    let mut daily_total = alloy_primitives::U256::ZERO;
    let mut hourly_count: u32 = 0;

    for tx in &txs {
        let ts = chrono::DateTime::parse_from_rfc3339(&tx.timestamp)
            .or_else(|_| chrono::DateTime::parse_from_str(&tx.timestamp, "%Y-%m-%dT%H:%M:%S%.fZ"))
            .ok();

        if let Some(ts) = ts {
            let ts_utc = ts.with_timezone(&chrono::Utc);

            if ts_utc > day_ago && tx.from.eq_ignore_ascii_case(from_address) {
                let val = tx.value.parse::<u128>().unwrap_or(0);
                daily_total += alloy_primitives::U256::from(val);
            }

            if ts_utc > hour_ago && tx.from.eq_ignore_ascii_case(from_address) {
                hourly_count += 1;
            }
        }
    }

    Some(TransactionHistory {
        daily_total,
        window_tx_count: hourly_count,
        window_secs: 3600,
    })
}
