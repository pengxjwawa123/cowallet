use axum::{
    extract::State,
    http::StatusCode,
    routing::post,
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::PgPool;

use crate::middleware::auth::Claims;
use crate::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/register", post(register_token))
        .route("/send", post(send_push))
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

fn err(status: StatusCode, msg: &str) -> (StatusCode, Json<ErrorResponse>) {
    (status, Json(ErrorResponse { error: msg.to_string() }))
}

#[derive(Debug, Deserialize)]
struct RegisterTokenRequest {
    token: String,
    platform: String,
    device_id: String,
}

async fn register_token(
    State(state): State<AppState>,
    claims: axum::Extension<Claims>,
    Json(req): Json<RegisterTokenRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = claims.0.sub.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user id in token"))?;

    if req.platform != "ios" && req.platform != "android" {
        return Err(err(StatusCode::BAD_REQUEST, "platform must be 'ios' or 'android'"));
    }

    sqlx::query(
        "INSERT INTO push_tokens (user_id, token, platform, device_id)
         VALUES ($1, $2, $3, $4)
         ON CONFLICT (token)
         DO UPDATE SET user_id = EXCLUDED.user_id, platform = EXCLUDED.platform,
                       device_id = EXCLUDED.device_id, updated_at = NOW()"
    )
    .bind(user_id)
    .bind(&req.token)
    .bind(&req.platform)
    .bind(&req.device_id)
    .execute(db)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    Ok(Json(json!({ "success": true })))
}

#[derive(Debug, Deserialize)]
struct SendPushRequest {
    user_id: String,
    title: String,
    body: String,
    data: serde_json::Value,
}

async fn send_push(
    State(state): State<AppState>,
    Json(req): Json<SendPushRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
    let db = state.require_db().map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "database not available"))?;
    let user_id: uuid::Uuid = req.user_id.parse()
        .map_err(|_| err(StatusCode::BAD_REQUEST, "invalid user_id"))?;

    let fcm_server_key = std::env::var("FCM_SERVER_KEY")
        .map_err(|_| err(StatusCode::SERVICE_UNAVAILABLE, "FCM not configured"))?;

    let tokens: Vec<(String, String)> = sqlx::query_as(
        "SELECT token, device_id FROM push_tokens WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_all(db)
    .await
    .map_err(|e| err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()))?;

    let mut sent_count = 0usize;
    for (token, _device_id) in &tokens {
        if send_fcm_push(&state.http, &fcm_server_key, token, &req.title, &req.body, &req.data).await.is_ok() {
            sent_count += 1;
        }
    }

    Ok(Json(json!({ "success": true, "sent_count": sent_count })))
}

async fn send_fcm_push(
    client: &reqwest::Client,
    server_key: &str,
    token: &str,
    title: &str,
    body: &str,
    data: &serde_json::Value,
) -> Result<(), String> {
    let payload = json!({
        "to": token,
        "notification": {
            "title": title,
            "body": body,
            "sound": "default",
            "badge": 1,
        },
        "data": data,
        "priority": "high",
        "content_available": true,
    });

    let response = client
        .post("https://fcm.googleapis.com/fcm/send")
        .header("Authorization", format!("key={}", server_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("FCM request failed: {}", e))?;

    if !response.status().is_success() {
        let error_body = response.text().await.unwrap_or_default();
        return Err(format!("FCM error: {}", error_body));
    }

    Ok(())
}

/// Helper to send MPC signing request push notification.
pub async fn send_mpc_signing_notification(
    db: &PgPool,
    http_client: &reqwest::Client,
    user_id: uuid::Uuid,
    session_id: &str,
    amount: &str,
    to_address: &str,
) {
    let fcm_server_key = match std::env::var("FCM_SERVER_KEY") {
        Ok(key) => key,
        Err(_) => return,
    };

    let tokens: Result<Vec<(String,)>, _> = sqlx::query_as(
        "SELECT token FROM push_tokens WHERE user_id = $1"
    )
    .bind(user_id)
    .fetch_all(db)
    .await;

    let tokens = match tokens {
        Ok(t) => t,
        Err(_) => return,
    };

    let data = json!({
        "type": "mpc_sign_request",
        "session_id": session_id,
        "amount": amount,
        "to": to_address,
    });

    for (token,) in &tokens {
        let _ = send_fcm_push(
            http_client,
            &fcm_server_key,
            token,
            "Signature Request",
            &format!("Approve transaction: {} to {}", amount, to_address),
            &data,
        )
        .await;
    }
}
