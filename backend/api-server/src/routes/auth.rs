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
        .route("/email/send-otp", post(send_email_otp))
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
struct SendEmailOtpRequest {
    email: String,
}

#[derive(Serialize)]
struct SendEmailOtpResponse {
    sent: bool,
    is_registered: bool,
    message: String,
}

/// Send OTP to email for registration verification.
async fn send_email_otp(
    State(state): State<AppState>,
    Json(body): Json<SendEmailOtpRequest>,
) -> Result<Json<SendEmailOtpResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Check if email has a completed wallet (user exists AND has at least one wallet with shard)
    let is_registered: bool = sqlx::query_as::<_, (uuid::Uuid,)>(
        "SELECT u.id FROM users u
         INNER JOIN shard_metadata s ON s.user_id = u.id
         WHERE u.email = $1
         LIMIT 1"
    )
    .bind(&body.email)
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .is_some();

    // If already registered with wallet, skip OTP — client should redirect to recovery flow
    if is_registered {
        return Ok(Json(SendEmailOtpResponse {
            sent: false,
            is_registered,
            message: "Account already registered. Please use recovery flow.".into(),
        }));
    }

    // Generate 6-digit OTP
    let otp = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let otp_hash = Sha256::digest(otp.as_bytes());
    let expires_at = Utc::now() + chrono::Duration::minutes(10);

    // Invalidate previous pending verifications for this email
    let _ = sqlx::query(
        "DELETE FROM email_verifications WHERE email = $1 AND NOT verified"
    )
    .bind(&body.email)
    .execute(db)
    .await;

    // Store verification record
    sqlx::query(
        "INSERT INTO email_verifications (email, otp_hash, expires_at) VALUES ($1, $2, $3)"
    )
    .bind(&body.email)
    .bind(otp_hash.as_slice())
    .bind(expires_at)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to store email verification: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Send OTP via email (AWS SES)
    if let Some(email_service) = &state.email {
        email_service.send_otp(&body.email, &otp).await.map_err(|e| {
            tracing::error!("Failed to send email OTP: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        tracing::warn!("⚠️  [NO SES] Email OTP not sent for {} (set SES_FROM_ADDRESS to enable)", body.email);
        #[cfg(debug_assertions)]
        tracing::debug!("DEV ONLY — OTP: {}", otp);
    }

    Ok(Json(SendEmailOtpResponse {
        sent: true,
        is_registered,
        message: format!("Verification code sent to {}", body.email),
    }))
}

#[derive(Deserialize)]
struct RegisterRequest {
    email: String,
    otp: String,
    device_id: String,
    #[serde(default)]
    force: bool,
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

    // Verify email OTP
    let otp_hash = Sha256::digest(body.otp.as_bytes());
    let verification: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM email_verifications
         WHERE email = $1 AND otp_hash = $2 AND NOT verified AND expires_at > NOW()
         ORDER BY created_at DESC LIMIT 1"
    )
    .bind(&body.email)
    .bind(otp_hash.as_slice())
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (verification_id,) = verification.ok_or(StatusCode::UNAUTHORIZED)?;

    // Mark as verified
    sqlx::query("UPDATE email_verifications SET verified = TRUE WHERE id = $1")
        .bind(verification_id)
        .execute(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Check if user exists and whether they have a completed wallet
    let existing: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT id FROM users WHERE email = $1"
    )
    .bind(&body.email)
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let user_id = if let Some((existing_id,)) = existing {
        // User exists — check if they have a completed wallet
        let has_wallet: bool = sqlx::query_as::<_, (uuid::Uuid,)>(
            "SELECT id FROM shard_metadata WHERE user_id = $1 LIMIT 1"
        )
        .bind(existing_id)
        .fetch_optional(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .is_some();

        if has_wallet && !body.force {
            return Err(StatusCode::CONFLICT);
        }

        if has_wallet && body.force {
            sqlx::query("DELETE FROM shard_metadata WHERE user_id = $1")
                .bind(existing_id)
                .execute(db)
                .await
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        }

        // Update device_id and reuse the user
        sqlx::query("UPDATE users SET device_id = $1 WHERE id = $2")
            .bind(&body.device_id)
            .bind(existing_id)
            .execute(db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

        existing_id
    } else {
        // New user
        let new_id = uuid::Uuid::new_v4();
        sqlx::query("INSERT INTO users (id, email, device_id) VALUES ($1, $2, $3)")
            .bind(new_id)
            .bind(&body.email)
            .bind(&body.device_id)
            .execute(db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        new_id
    };

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
/// Returns a consistent response regardless of whether the user exists (prevents enumeration).
async fn initiate_recovery(
    State(state): State<AppState>,
    Json(body): Json<InitiateRecoveryRequest>,
) -> Result<Json<InitiateRecoveryResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Always generate a session ID (returned even if user doesn't exist to prevent enumeration)
    let recovery_session_id = uuid::Uuid::new_v4();

    // Verify user exists and has a server shard
    let user_row: Option<(uuid::Uuid,)> = sqlx::query_as(
        "SELECT u.id FROM users u
         INNER JOIN shard_metadata s ON s.user_id = u.id AND s.location = 'server'
         WHERE u.email = $1
         LIMIT 1"
    )
    .bind(&body.email)
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // If user doesn't exist or has no shard, return success-like response (anti-enumeration)
    let Some((user_id,)) = user_row else {
        return Ok(Json(InitiateRecoveryResponse {
            recovery_session_id: recovery_session_id.to_string(),
            otp_sent: true,
            message: "If an account exists, a recovery code was sent to your email.".into(),
        }));
    };

    // Check for 30-minute cooldown after locked sessions
    let has_recent_lock: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM recovery_sessions
         WHERE user_id = $1 AND status = 'locked'
         AND created_at > NOW() - INTERVAL '30 minutes')"
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);

    if has_recent_lock {
        // Return 423 Locked — user must wait before retrying
        return Err(StatusCode::LOCKED);
    }

    // Invalidate all previous pending recovery sessions for this user
    let _ = sqlx::query(
        "UPDATE recovery_sessions SET status = 'expired' WHERE user_id = $1 AND status = 'pending'"
    )
    .bind(user_id)
    .execute(db)
    .await;

    // Generate OTP (6-digit code)
    let otp = format!("{:06}", rand::random::<u32>() % 1_000_000);
    let expires_at = Utc::now() + chrono::Duration::minutes(10);

    // Store recovery session with attempt counter
    sqlx::query(
        "INSERT INTO recovery_sessions (id, user_id, otp_hash, expires_at, status, attempts)
         VALUES ($1, $2, $3, $4, 'pending', 0)"
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

    if let Some(email_service) = &state.email {
        email_service.send_otp(&body.email, &otp).await.map_err(|e| {
            tracing::error!("Failed to send recovery OTP email: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    } else {
        tracing::warn!("⚠️  [NO SES] Recovery OTP not sent for {} (set SES_FROM_ADDRESS to enable)", body.email);
        #[cfg(debug_assertions)]
        tracing::debug!("DEV ONLY — OTP: {}", otp);
    }
    tracing::info!("Recovery initiated for user {} (session: {})", user_id, recovery_session_id);

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
        message: "If an account exists, a recovery code was sent to your email.".into(),
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
    /// Feldman commitment: G * (lambda_1 * s_1), compressed SEC1 hex.
    /// Client verifies: server_commitment + G*(lambda_2 * backup_shard) == PublicKey.
    server_commitment_hex: String,
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

    // Fetch recovery session with atomic attempt increment
    let session: Option<(uuid::Uuid, Vec<u8>, chrono::DateTime<Utc>, String, i32)> = sqlx::query_as(
        "UPDATE recovery_sessions SET attempts = COALESCE(attempts, 0) + 1
         WHERE id = $1
         RETURNING user_id, otp_hash, expires_at, status, attempts"
    )
    .bind(recovery_session_id)
    .fetch_optional(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let (user_id, otp_hash, expires_at, status, attempts) =
        session.ok_or(StatusCode::NOT_FOUND)?;

    // Check expiration
    if Utc::now() > expires_at {
        return Err(StatusCode::GONE);
    }

    // Check status (must be pending — blocks reuse after success)
    if status != "pending" {
        return Err(StatusCode::CONFLICT);
    }

    // Brute-force protection: lock after 5 failed attempts
    if attempts > 5 {
        let _ = sqlx::query("UPDATE recovery_sessions SET status = 'locked' WHERE id = $1")
            .bind(recovery_session_id)
            .execute(db)
            .await;
        return Err(StatusCode::TOO_MANY_REQUESTS);
    }

    // Verify OTP with constant-time comparison
    let provided_hash = sha2::Sha256::digest(body.otp.as_bytes());
    let otp_valid = provided_hash.as_slice().len() == otp_hash.len()
        && provided_hash
            .as_slice()
            .iter()
            .zip(otp_hash.iter())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0;

    if !otp_valid {
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

    // Mark session as completed (single-use, prevents replay)
    sqlx::query("UPDATE recovery_sessions SET status = 'completed' WHERE id = $1")
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

    // Create recovery reshare MPC session in DB.
    // Participants: Server (1) + Backup (2). Target: Device (0).
    let session_id = uuid::Uuid::new_v4();
    let parties = vec![1i16, 2i16]; // Server + Backup (the available old-share holders)
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

    // Initialize server's reshare protocol in recovery mode.
    // The server uses its shard (Party 1) with Lagrange correction to produce
    // evaluations that will reconstruct Party 0's shard when combined with backup's contribution.
    if let Err(e) = mpc_participant
        .on_session_created(session_id, user_id, "reshare", &parties, threshold, None)
        .await
    {
        tracing::error!("Server reshare init failed for session {}: {}", session_id, e);
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    // Fetch server's Round 1 reshare messages (addressed to Party 0 — the target)
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

    // Serialize messages as JSON strings matching ProtocolMessage struct
    let server_reshare_messages_json: Vec<String> = messages
        .into_iter()
        .map(|(from, to, round, payload)| {
            serde_json::to_string(&serde_json::json!({
                "session_id": session_id.to_string(),
                "from": from,
                "to": to,
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

    // Compute Feldman commitment G*(lambda_1 * s_1) for client-side backup shard verification
    let server_commitment_hex = match mpc_participant.compute_recovery_commitment(user_id).await {
        Ok(bytes) => hex::encode(&bytes),
        Err(e) => {
            tracing::error!("Failed to compute recovery commitment: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

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
        server_commitment_hex,
    }))
}
