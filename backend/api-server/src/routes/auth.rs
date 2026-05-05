use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::middleware::audit::{AuditLog, AuditResult};
use crate::middleware::auth::{Claims, issue_token_pair, blacklist_token, refresh_access_token, TokenPair, verify_token_unchecked};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .route("/session", get(session_info))
        .route("/audit-log", get(audit_log))
}

#[derive(Deserialize)]
pub struct AuditLogQuery {
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub action: Option<String>,
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: Option<String>,
    device_id: String,
}

#[derive(Serialize)]
struct AuthResponse {
    token: String,
    refresh_token: String,
    expires_in: usize,
    token_type: &'static str,
    user_id: String,
}

#[derive(Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

#[derive(Deserialize)]
struct LogoutRequest {
    all_devices: Option<bool>,
}

async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let user_id = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO users (id, email, device_id) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(&body.email)
        .bind(&body.device_id)
        .execute(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let token_pair = issue_token_pair(&user_id.to_string(), &body.device_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "auth.register",
            AuditResult::Success,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({ "device_id": body.device_id })),
        )
        .await;

    Ok(Json(AuthResponse {
        token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        expires_in: token_pair.expires_in,
        token_type: token_pair.token_type,
        user_id: user_id.to_string(),
    }))
}

#[derive(Deserialize)]
struct LoginRequest {
    device_id: String,
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let result: Result<(uuid::Uuid,), StatusCode> = sqlx::query_as("SELECT id FROM users WHERE device_id = $1")
        .bind(&body.device_id)
        .fetch_one(db)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED);

    let user_id = match result {
        Ok((id,)) => id,
        Err(e) => {
            // Audit log - login failure
            let _ = state
                .audit_logger
                .log_with_details(
                    uuid::Uuid::nil(),
                    "auth.login",
                    AuditResult::Denied,
                    None,
                    None,
                    None,
                    None,
                    Some(serde_json::json!({ "device_id": body.device_id })),
                )
                .await;
            return Err(e);
        }
    };

    let token_pair = issue_token_pair(&user_id.to_string(), &body.device_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Audit log - login success
    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "auth.login",
            AuditResult::Success,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({ "device_id": body.device_id })),
        )
        .await;

    Ok(Json(AuthResponse {
        token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        expires_in: token_pair.expires_in,
        token_type: token_pair.token_type,
        user_id: user_id.to_string(),
    }))
}

async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<AuthResponse>, StatusCode> {
    let db = state
        .db
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let claims = verify_token_unchecked(&body.refresh_token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let token_pair = refresh_access_token(&db, &body.refresh_token, &claims.device_id).await?;

    let user_id = claims.sub.parse().unwrap_or(uuid::Uuid::nil());
    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "auth.refresh",
            AuditResult::Success,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({ "device_id": claims.device_id })),
        )
        .await;

    Ok(Json(AuthResponse {
        token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        expires_in: token_pair.expires_in,
        token_type: token_pair.token_type,
        user_id: claims.sub,
    }))
}

async fn logout(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(body): Json<LogoutRequest>,
) -> Result<StatusCode, StatusCode> {
    let db = state
        .db
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let user_id = claims.0.sub.parse().unwrap_or(uuid::Uuid::nil());

    blacklist_token(
        &db,
        &claims.0.jti,
        &claims.0.sub,
        claims.0.exp,
        Some("User logout".to_string()),
    )
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "auth.logout",
            AuditResult::Success,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "device_id": claims.0.device_id,
                "all_devices": body.all_devices.unwrap_or(false)
            })),
        )
        .await;

    Ok(StatusCode::OK)
}

async fn session_info(
    claims: Option<axum::Extension<Claims>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let claims = claims.ok_or(StatusCode::UNAUTHORIZED)?.0;
    Ok(Json(serde_json::json!({
        "user_id": claims.sub,
        "device_id": claims.device_id,
        "expires_at": claims.exp,
    })))
}

async fn audit_log(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Query(query): Query<AuditLogQuery>,
) -> Result<Json<Vec<AuditLog>>, StatusCode> {
    let user_id: uuid::Uuid = claims
        .0
        .sub
        .parse()
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let logs = if let Some(action) = &query.action {
        state
            .audit_logger
            .get_logs_by_action(user_id, action, limit)
            .await
    } else {
        state
            .audit_logger
            .get_user_logs(user_id, limit, offset)
            .await
    };

    match logs {
        Ok(logs) => Ok(Json(logs)),
        Err(e) => {
            tracing::error!("Failed to fetch audit logs: {}", e);
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}
