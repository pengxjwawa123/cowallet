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
        .route("/session/:id", get(get_session))
        .route("/session/:id", delete(abort_session))
        .route("/session/:id/msg", post(send_message))
        .route("/session/:id/msg", get(recv_messages))
}

#[derive(Deserialize)]
struct CreateSessionRequest {
    session_type: String,
    parties: Vec<i16>,
    threshold: Option<i16>,
}

#[derive(Serialize)]
struct SessionResponse {
    session_id: String,
    status: String,
    current_round: i16,
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
            (SessionStatus::Pending, SessionStatus::Expired) => true,
            (SessionStatus::Active, SessionStatus::Completed) => true,
            (SessionStatus::Active, SessionStatus::Failed) => true,
            (SessionStatus::Active, SessionStatus::Expired) => true,
            _ => false,
        }
    }
}

/// Validate that a party index is within the allowed range
fn validate_party_index(party: i16, parties: &[i16]) -> Result<(), StatusCode> {
    if !parties.contains(&party) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(())
}

/// Validate session is not expired and in a valid state for messaging
fn validate_session_state(
    status: &str,
    expires_at: Option<chrono::DateTime<Utc>>,
) -> Result<SessionStatus, StatusCode> {
    let session_status = SessionStatus::from_str(status)
        .ok_or(StatusCode::BAD_REQUEST)?;

    // Check if session is in a terminal state
    if matches!(
        session_status,
        SessionStatus::Completed | SessionStatus::Failed | SessionStatus::Expired
    ) {
        return Err(StatusCode::GONE);
    }

    // Check expiration
    if let Some(expires) = expires_at {
        if Utc::now() > expires {
            return Err(StatusCode::GONE);
        }
    }

    Ok(session_status)
}

/// Calculate HMAC for message integrity
fn calculate_message_hmac(
    session_id: &uuid::Uuid,
    from_party: i16,
    to_party: i16,
    round: i16,
    payload: &[u8],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update(from_party.to_be_bytes());
    hasher.update(to_party.to_be_bytes());
    hasher.update(round.to_be_bytes());
    hasher.update(payload);
    let result = hasher.finalize();
    hex::encode(result)
}

async fn create_session(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let session_id = uuid::Uuid::new_v4();
    let threshold = body.threshold.unwrap_or(2);
    let total_parties = body.parties.len() as i16;

    // Validate parties
    if body.parties.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Validate threshold
    if threshold < 1 || threshold > total_parties {
        return Err(StatusCode::BAD_REQUEST);
    }

    let initiator_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let db_session_type = match body.session_type.as_str() {
        "keygen" => "dkg",
        other => other,
    };

    sqlx::query(
        "INSERT INTO mpc_sessions
         (id, session_type, initiator_id, parties, threshold, total_parties, status, current_round)
         VALUES ($1, $2, $3, $4, $5, $6, 'pending', 0)"
    )
    .bind(session_id)
    .bind(db_session_type)
    .bind(initiator_id)
    .bind(&body.parties)
    .bind(threshold)
    .bind(total_parties)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!("Created MPC session {} for user {}", session_id, initiator_id);

    Ok(Json(SessionResponse {
        session_id: session_id.to_string(),
        status: "pending".into(),
        current_round: 0,
        last_activity: None,
    }))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let row: (String, i16, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
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

async fn abort_session(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

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
struct SendMessageRequest {
    from_party: i16,
    to_party: i16,
    round: i16,
    payload: Vec<u8>,
    /// Optional HMAC for message integrity verification
    hmac: Option<String>,
}

#[derive(Serialize)]
struct SendMessageResponse {
    message_id: i64,
    verified: bool,
}

async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Fetch session details
    let session: (String, Vec<i16>, i16, Option<chrono::DateTime<Utc>>) = sqlx::query_as(
        "SELECT status, parties, current_round, expires_at FROM mpc_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let status = session.0;
    let parties = session.1;
    let current_round = session.2;
    let expires_at = session.3;

    // Validate session state
    let session_status = validate_session_state(&status, expires_at)?;

    // Validate party indices
    validate_party_index(body.from_party, &parties)?;
    validate_party_index(body.to_party, &parties)?;

    // Validate round (must be >= current round)
    if body.round < current_round {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Verify HMAC if provided
    let hmac_verified = if let Some(provided_hmac) = body.hmac.as_ref() {
        let expected_hmac = calculate_message_hmac(
            &session_id,
            body.from_party,
            body.to_party,
            body.round,
            &body.payload,
        );
        provided_hmac == &expected_hmac
    } else {
        false
    };

    // Transition to active state if pending
    let new_status = if session_status == SessionStatus::Pending {
        "active"
    } else {
        &status
    };

    // Update session activity and optionally status
    sqlx::query(
        "UPDATE mpc_sessions
         SET last_activity = NOW(),
             current_round = GREATEST(current_round, $1),
             status = $2
         WHERE id = $3"
    )
    .bind(body.round)
    .bind(new_status)
    .bind(session_id)
    .execute(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Insert message
    let message_id: (i64,) = sqlx::query_as(
        "INSERT INTO mpc_messages (session_id, from_party, to_party, round, payload, verified)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id"
    )
    .bind(session_id)
    .bind(body.from_party)
    .bind(body.to_party)
    .bind(body.round)
    .bind(&body.payload)
    .bind(hmac_verified)
    .fetch_one(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to insert message: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::debug!(
        "MPC message {} from party {} to party {} in session {} (round {})",
        message_id.0,
        body.from_party,
        body.to_party,
        session_id,
        body.round
    );

    Ok(Json(SendMessageResponse {
        message_id: message_id.0,
        verified: hmac_verified,
    }))
}

#[derive(Serialize)]
struct MessageResponse {
    id: i64,
    from_party: i16,
    to_party: i16,
    round: i16,
    payload: Vec<u8>,
    verified: bool,
    created_at: String,
}

async fn recv_messages(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<MessageResponse>>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Verify session exists
    let _session_exists: (i64,) = sqlx::query_as(
        "SELECT 1 FROM mpc_sessions WHERE id = $1 LIMIT 1"
    )
    .bind(session_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let rows: Vec<(i64, i16, i16, i16, Vec<u8>, bool, chrono::DateTime<Utc>)> = sqlx::query_as(
        "SELECT id, from_party, to_party, round, payload, verified, created_at
         FROM mpc_messages
         WHERE session_id = $1
         ORDER BY round, created_at"
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch messages: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let messages: Vec<MessageResponse> = rows
        .into_iter()
        .map(|(id, from_party, to_party, round, payload, verified, created_at)| {
            MessageResponse {
                id,
                from_party,
                to_party,
                round,
                payload,
                verified,
                created_at: created_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(Json(messages))
}
