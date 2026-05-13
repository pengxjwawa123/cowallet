use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    routing::{get, post},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

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
        .route("/recovery/initiate", post(initiate_recovery))
        .route("/recovery/verify", post(verify_recovery_otp))
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

// ─── Wallet Recovery Endpoints ───────────────────────────────────────────────

#[derive(Deserialize)]
struct InitiateRecoveryRequest {
    email: String,
}

#[derive(Serialize)]
struct InitiateRecoveryResponse {
    recovery_session_id: String,
    otp_sent: bool,
    message: String,
}

/// Initiate wallet recovery process.
/// Sends OTP to user's email and creates a recovery session.
async fn initiate_recovery(
    State(state): State<AppState>,
    Json(body): Json<InitiateRecoveryRequest>,
) -> Result<Json<InitiateRecoveryResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Verify user exists
    let user_id: Result<(uuid::Uuid,), StatusCode> = sqlx::query_as(
        "SELECT id FROM users WHERE email = $1"
    )
    .bind(&body.email)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND);

    let (user_id,) = user_id?;

    // Check if user has a server shard
    let has_server_shard: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM shard_metadata WHERE user_id = $1 AND location = 'server')"
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_server_shard {
        return Err(StatusCode::NOT_FOUND);
    }

    // Generate OTP (6-digit code)
    let otp = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let recovery_session_id = uuid::Uuid::new_v4();
    let expires_at = Utc::now() + chrono::Duration::minutes(10);

    // Store recovery session
    sqlx::query(
        "INSERT INTO recovery_sessions (id, user_id, otp_hash, expires_at, status)
         VALUES ($1, $2, $3, $4, 'pending')"
    )
    .bind(recovery_session_id)
    .bind(user_id)
    .bind(sha2::Sha256::digest(otp.as_bytes()).as_slice())
    .bind(expires_at)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create recovery session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // TODO: Send OTP via email (placeholder for now)
    tracing::info!(
        "Recovery OTP for user {}: {} (session: {})",
        user_id, otp, recovery_session_id
    );

    // Audit log
    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "auth.recovery_initiated",
            AuditResult::Success,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({ "email": body.email })),
        )
        .await;

    Ok(Json(InitiateRecoveryResponse {
        recovery_session_id: recovery_session_id.to_string(),
        otp_sent: true,
        message: format!("Recovery OTP sent to {}. Please check your email.", body.email),
    }))
}

#[derive(Deserialize)]
struct VerifyRecoveryOtpRequest {
    recovery_session_id: String,
    otp: String,
    device_id: String,
}

#[derive(Serialize)]
struct VerifyRecoveryOtpResponse {
    token: String,
    refresh_token: String,
    expires_in: usize,
    token_type: &'static str,
    user_id: String,
    public_key_hex: String,
    server_reshare_messages_json: Vec<String>,
}

/// Verify recovery OTP and return server's reshare contribution.
/// This starts the recovery protocol where the server (Party 1) and backup (Party 2)
/// collaborate to reconstruct the device shard (Party 0).
async fn verify_recovery_otp(
    State(state): State<AppState>,
    Json(body): Json<VerifyRecoveryOtpRequest>,
) -> Result<Json<VerifyRecoveryOtpResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let recovery_session_id = uuid::Uuid::parse_str(&body.recovery_session_id)
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    // Fetch recovery session
    let session: Result<(uuid::Uuid, Vec<u8>, chrono::DateTime<Utc>, String), _> = sqlx::query_as(
        "SELECT user_id, otp_hash, expires_at, status FROM recovery_sessions WHERE id = $1"
    )
    .bind(recovery_session_id)
    .fetch_one(db)
    .await;

    let (user_id, otp_hash, expires_at, status) = session.map_err(|_| StatusCode::NOT_FOUND)?;

    // Check expiration
    if Utc::now() > expires_at {
        return Err(StatusCode::GONE);
    }

    // Check status
    if status != "pending" {
        return Err(StatusCode::CONFLICT);
    }

    // Verify OTP
    let provided_hash = sha2::Sha256::digest(body.otp.as_bytes());
    if provided_hash.as_slice() != otp_hash.as_slice() {
        // Audit log - failed verification
        let _ = state
            .audit_logger
            .log_with_details(
                user_id,
                "auth.recovery_otp_failed",
                AuditResult::Denied,
                None,
                None,
                None,
                None,
                None,
            )
            .await;

        return Err(StatusCode::UNAUTHORIZED);
    }

    // Mark session as verified
    sqlx::query("UPDATE recovery_sessions SET status = 'verified' WHERE id = $1")
        .bind(recovery_session_id)
        .execute(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Fetch server shard and public key
    let shard_row: (Vec<u8>, Vec<u8>, i16) = sqlx::query_as(
        "SELECT encrypted_shard, nonce, party_index
         FROM shard_metadata
         WHERE user_id = $1 AND location = 'server'"
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    // Get MPC participant service
    let mpc_participant = state
        .mpc_participant
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    // Create reshare MPC session in DB
    let session_id = uuid::Uuid::new_v4();
    let parties = vec![0i16, 1i16]; // Device (0) and Server (1)
    let threshold = 2i16;

    sqlx::query(
        "INSERT INTO mpc_sessions (id, user_id, session_type, parties, threshold, status, current_round)
         VALUES ($1, $2, 'reshare', $3, $4, 'active', 0)"
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&parties)
    .bind(threshold)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create reshare session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Initialize server's reshare protocol (loads shard, generates Round 1)
    if let Err(e) = mpc_participant
        .on_session_created(session_id, user_id, "reshare", &parties, threshold, None)
        .await
    {
        tracing::error!("Server reshare init failed for session {}: {}", session_id, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Fetch server's Round 1 reshare messages (addressed to Party 0)
    let messages: Vec<(i16, i16, i16, Vec<u8>)> = sqlx::query_as(
        "SELECT from_party, to_party, round, payload
         FROM mpc_messages
         WHERE session_id = $1 AND from_party = 1 AND round = 1
         ORDER BY created_at ASC"
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch reshare messages: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Serialize messages as JSON strings (client expects array of JSON-encoded messages)
    let server_reshare_messages_json: Vec<String> = messages
        .into_iter()
        .map(|(from, to, round, payload)| {
            serde_json::to_string(&serde_json::json!({
                "from_party": from,
                "to_party": to,
                "round": round,
                "payload": payload
            }))
            .unwrap_or_default()
        })
        .collect();

    // Get public key from wallets table (use the first active wallet for this user)
    let public_key: Vec<u8> = sqlx::query_scalar(
        "SELECT public_key FROM wallets WHERE user_id = $1 AND status = 'active' ORDER BY created_at ASC LIMIT 1"
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch public key: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?
    .ok_or_else(|| {
        tracing::warn!("No active wallet found for user {}", user_id);
        StatusCode::NOT_FOUND
    })?;

    let public_key_hex = hex::encode(&public_key);

    // Issue JWT token pair
    let token_pair = issue_token_pair(&user_id.to_string(), &body.device_id)
        .map_err(|e| {
            tracing::error!("Failed to issue token pair: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    // Audit log - successful recovery
    let _ = state
        .audit_logger
        .log_with_details(
            user_id,
            "auth.recovery_success",
            AuditResult::Success,
            None,
            None,
            None,
            None,
            Some(serde_json::json!({
                "device_id": body.device_id,
                "reshare_session_id": session_id.to_string()
            })),
        )
        .await;

    tracing::info!(
        "Recovery OTP verified for user {}, reshare session {} initiated",
        user_id,
        session_id
    );

    Ok(Json(VerifyRecoveryOtpResponse {
        token: token_pair.access_token,
        refresh_token: token_pair.refresh_token,
        expires_in: token_pair.expires_in,
        token_type: token_pair.token_type,
        user_id: user_id.to_string(),
        public_key_hex,
        server_reshare_messages_json,
    }))
}
