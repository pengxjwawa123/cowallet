use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::middleware::auth::{Claims, issue_token, verify_token};
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/session", get(session_info))
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: Option<String>,
    device_id: String,
}

#[derive(Serialize)]
struct AuthResponse {
    token: String,
    user_id: String,
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

    let token = issue_token(&user_id.to_string(), &body.device_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AuthResponse {
        token,
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

    let row: (uuid::Uuid,) = sqlx::query_as("SELECT id FROM users WHERE device_id = $1")
        .bind(&body.device_id)
        .fetch_one(db)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let token = issue_token(&row.0.to_string(), &body.device_id)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(AuthResponse {
        token,
        user_id: row.0.to_string(),
    }))
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
