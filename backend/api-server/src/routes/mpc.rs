use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Extension,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Sha256, Digest};

use crate::middleware::auth::Claims;
use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/session", post(create_session))
        .route("/session/{id}", get(get_session))
        .route("/session/{id}", delete(abort_session))
        .route("/session/{id}/msg", post(send_message))
        .route("/session/{id}/msg", get(recv_messages))
}

#[derive(Deserialize)]
pub(crate) struct CreateSessionRequest {
    session_type: String,
    parties: Vec<i16>,
    threshold: Option<i16>,
}

#[derive(Serialize)]
pub(crate) struct SessionResponse {
    session_id: String,
    status: String,
    current_round: i32,
    last_activity: Option<String>,
}

/// Valid session states
#[derive(Debug, Clone, Copy, PartialEq)]
enum SessionStatus {
    Pending,
    Active,
    Completed,
    Failed,
    Expired,
}

impl SessionStatus {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(SessionStatus::Pending),
            "active" => Some(SessionStatus::Active),
            "completed" => Some(SessionStatus::Completed),
            "failed" => Some(SessionStatus::Failed),
            "expired" => Some(SessionStatus::Expired),
            _ => None,
        }
    }

    fn to_str(self) -> &'static str {
        match self {
            SessionStatus::Pending => "pending",
            SessionStatus::Active => "active",
            SessionStatus::Completed => "completed",
            SessionStatus::Failed => "failed",
            SessionStatus::Expired => "expired",
        }
    }

    /// Check if state transition is valid
    fn can_transition_to(self, next: SessionStatus) -> bool {
        match (self, next) {
            (SessionStatus::Pending, SessionStatus::Active) => true,
            (SessionStatus::Pending, SessionStatus::Failed) => true,
            (SessionStatus::Active, SessionStatus::Completed) => true,
            (SessionStatus::Active, SessionStatus::Failed) => true,
            (_, SessionStatus::Expired) => true,
            (current, next) if current == next => true, // Same state is fine
            _ => false,
        }
    }
}

/// Create a new MPC session
pub async fn create_session(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let valid_types = ["dkg", "keygen", "presign", "sign", "reshare"];
    if !valid_types.contains(&body.session_type.as_str()) {
        tracing::warn!("Invalid session_type: {}", body.session_type);
        return Err(StatusCode::BAD_REQUEST);
    }

    let session_id = uuid::Uuid::new_v4();
    let threshold = body.threshold.unwrap_or(2);

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let result = sqlx::query(
        "INSERT INTO mpc_sessions (id, user_id, session_type, parties, threshold, status, current_round)
         VALUES ($1, $2, $3, $4, $5, 'pending', 0)"
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&body.session_type)
    .bind(&body.parties)
    .bind(threshold)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create MPC session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!("Created MPC session {} for user {}", session_id, claims.sub);

    Ok(Json(SessionResponse {
        session_id: session_id.to_string(),
        status: "pending".to_string(),
        current_round: 0,
        last_activity: None,
    }))
}

/// Get session status
pub async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let row: (String, i32, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT status, current_round, last_activity FROM mpc_sessions WHERE id = $1"
    )
    .bind(id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(SessionResponse {
        session_id: id.to_string(),
        status: row.0,
        current_round: row.1,
        last_activity: row.2.map(|t| t.to_rfc3339()),
    }))
}

/// Abort a session
pub async fn abort_session(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let result = sqlx::query(
        "UPDATE mpc_sessions SET status = 'failed' WHERE id = $1 AND status IN ('pending', 'active')"
    )
    .bind(id)
    .execute(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if result.rows_affected() == 0 {
        return Err(StatusCode::GONE);
    }

    tracing::info!("Aborted MPC session {}", id);

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub(crate) struct SendMessageRequest {
    from_party: i16,
    to_party: i16,
    round: i16,
    payload: Vec<u8>,
    /// Optional HMAC for message integrity verification
    hmac: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct SendMessageResponse {
    message_id: i64,
    verified: bool,
}

/// Send a message to another party in the session
pub async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Fetch session details
    let session: (String, Vec<i16>, i32, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT status, parties, current_round, expires_at FROM mpc_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let _status = session.0;
    let parties = session.1;
    let current_round = session.2 as i16;

    // Validate party indices
    if !parties.contains(&body.from_party) || !parties.contains(&body.to_party) {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate round (must be >= current round)
    if body.round < current_round {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify HMAC if provided
    let verified = if let Some(hmac) = &body.hmac {
        // In production, use the session's shared secret
        let secret = session_id.to_string();
        let mut mac = Sha256::new();
        mac.update(secret.as_bytes());
        mac.update(&body.payload);
        let expected = hex::encode(mac.finalize());
        hmac == &expected
    } else {
        false
    };

    // Store message
    let message_id: i64 = sqlx::query_scalar(
        "INSERT INTO mpc_messages (session_id, from_party, to_party, round, payload, verified)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id"
    )
    .bind(session_id)
    .bind(body.from_party)
    .bind(body.to_party)
    .bind(body.round)
    .bind(&body.payload)
    .bind(verified)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Update session activity
    sqlx::query(
        "UPDATE mpc_sessions SET last_activity = NOW() WHERE id = $1"
    )
    .bind(session_id)
    .execute(db)
    .await
    .ok();

    Ok(Json(SendMessageResponse {
        message_id,
        verified,
    }))
}

#[derive(Serialize)]
pub(crate) struct MessageResponse {
    id: i64,
    from_party: i16,
    to_party: i16,
    round: i16,
    payload: Vec<u8>,
    verified: bool,
    created_at: String,
}

/// Receive messages for a session (polling-based)
pub async fn recv_messages(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<MessageResponse>>, StatusCode> {
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Verify session exists
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM mpc_sessions WHERE id = $1)")
        .bind(session_id)
        .fetch_one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !exists {
        return Err(StatusCode::NOT_FOUND);
    }

    let messages: Vec<(i64, i16, i16, i16, Vec<u8>, bool, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, from_party, to_party, round, payload, verified, created_at
         FROM mpc_messages
         WHERE session_id = $1
         ORDER BY round ASC, created_at ASC"
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(messages.into_iter().map(|m| MessageResponse {
        id: m.0,
        from_party: m.1,
        to_party: m.2,
        round: m.3,
        payload: m.4,
        verified: m.5,
        created_at: m.6.to_rfc3339(),
    }).collect()))
}
