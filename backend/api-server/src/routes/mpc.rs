use axum::{
    Json, Router,
    extract::{Path, Query, State},
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
        .route("/session/{id}/backup-contribution", get(get_backup_contribution))
        .route("/presign/status", get(presign_status))
        .route("/presign/generate", post(presign_generate))
}

#[derive(Deserialize)]
pub(crate) struct CreateSessionRequest {
    session_type: String,
    parties: Vec<i16>,
    threshold: Option<i16>,
    /// Optional wallet ID for multi-wallet support.
    /// When provided, the session is associated with a specific wallet
    /// and uses that wallet's key share for signing.
    wallet_id: Option<uuid::Uuid>,
}

#[derive(Serialize)]
pub(crate) struct SessionResponse {
    session_id: String,
    status: String,
    current_round: i32,
    last_activity: Option<String>,
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

    sqlx::query(
        "INSERT INTO mpc_sessions (id, user_id, session_type, parties, threshold, status, current_round, wallet_id)
         VALUES ($1, $2, $3, $4, $5, 'active', 0, $6)"
    )
    .bind(session_id)
    .bind(user_id)
    .bind(&body.session_type)
    .bind(&body.parties)
    .bind(threshold)
    .bind(body.wallet_id)
    .execute(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create MPC session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!("Created MPC session {} for user {}", session_id, claims.sub);

    // Notify the server MPC participant to join this session
    if let Some(participant) = &state.mpc_participant {
        if let Err(e) = participant.on_session_created(
            session_id,
            user_id,
            &body.session_type,
            &body.parties,
            threshold,
            body.wallet_id,
        ).await {
            tracing::error!("Server participant failed to join session {}: {}", session_id, e);
            // Non-fatal: session still exists, client can retry
        }
    }

    Ok(Json(SessionResponse {
        session_id: session_id.to_string(),
        status: "active".to_string(),
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
    Extension(claims): Extension<Claims>,
    Path(session_id): Path<uuid::Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> Result<Json<SendMessageResponse>, StatusCode> {
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    // Fetch session details
    let session: (String, Vec<i16>, i32, uuid::Uuid) = sqlx::query_as(
        "SELECT status, parties, current_round, user_id FROM mpc_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let status = &session.0;
    let parties = &session.1;
    let current_round = session.2 as i16;
    let session_user_id = session.3;

    // Session must be active
    if status != "active" {
        return Err(StatusCode::GONE);
    }

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
    .map_err(|e| {
        tracing::error!("Failed to store message: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    // Update session activity and round
    sqlx::query(
        "UPDATE mpc_sessions SET last_activity = NOW(), current_round = GREATEST(current_round, $2)
         WHERE id = $1"
    )
    .bind(session_id)
    .bind(body.round as i32)
    .execute(db)
    .await
    .ok();

    // If this message is addressed to the server (Party 1), trigger the participant
    if body.to_party == 1 {
        if let Some(participant) = &state.mpc_participant {
            match participant.on_message_received(
                session_id,
                body.from_party,
                body.round,
                &body.payload,
            ).await {
                Ok(responses) => {
                    tracing::info!(
                        "Server participant processed message for session {} round {}, {} responses",
                        session_id, body.round, responses.len()
                    );
                    // Publish response messages to NATS so the client's WS gets them in real-time.
                    // Messages are already stored in DB by the participant's store_outbound_message.
                    if let Some(nats) = &state.nats {
                        for (from, to, payload) in responses {
                            // to == -1 means broadcast; send to the requesting party
                            let target_party = if to == -1 { body.from_party } else { to };
                            let response_msg = serde_json::json!({
                                "from_party": from,
                                "to_party": target_party,
                                "round": body.round + 1,
                                "payload": payload,
                            });
                            let subject = format!("cowallet.mpc.{}.{}", session_id, target_party);
                            if let Ok(data) = serde_json::to_vec(&response_msg) {
                                if let Err(e) = nats.publish(subject.clone(), data.into()).await {
                                    tracing::warn!("NATS publish to {} failed: {}", subject, e);
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Server participant error for session {} round {}: {}",
                        session_id, body.round, e
                    );
                }
            }
        }
    }

    Ok(Json(SendMessageResponse {
        message_id,
        verified,
    }))
}

#[derive(Deserialize)]
pub(crate) struct RecvQuery {
    /// Filter messages addressed to this party (required).
    party: Option<i16>,
    /// Only return messages after this ID (for polling).
    after_id: Option<i64>,
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

/// Receive messages for a session (polling-based).
/// Query params:
///   ?party=0  — filter messages addressed to this party (or broadcast)
///   ?after_id=5 — only return messages with id > this value
pub async fn recv_messages(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
    Query(query): Query<RecvQuery>,
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

    let after_id = query.after_id.unwrap_or(0);

    let messages: Vec<(i64, i16, i16, i16, Vec<u8>, bool, chrono::DateTime<Utc>)> = if let Some(party) = query.party {
        // Filter: messages addressed to this party OR broadcast (0xFFFF = 65535 as i16 = -1)
        sqlx::query_as(
            "SELECT id, from_party, to_party, round, payload, verified, created_at
             FROM mpc_messages
             WHERE session_id = $1 AND id > $2 AND (to_party = $3 OR to_party = -1)
             ORDER BY round ASC, created_at ASC"
        )
        .bind(session_id)
        .bind(after_id)
        .bind(party)
        .fetch_all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    } else {
        // No filter — return all (backwards compatible)
        sqlx::query_as(
            "SELECT id, from_party, to_party, round, payload, verified, created_at
             FROM mpc_messages
             WHERE session_id = $1 AND id > $2
             ORDER BY round ASC, created_at ASC"
        )
        .bind(session_id)
        .bind(after_id)
        .fetch_all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    };

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

/// Get the server's backup contribution for a completed DKG session.
/// Returns the 32-byte f_server(3) scalar for the client to combine with f_device(3).
/// This is a single-use endpoint: the contribution is removed after fetching.
pub async fn get_backup_contribution(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<u8>>, StatusCode> {
    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify the session exists and belongs to this user
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let session_user: Option<(uuid::Uuid, String)> = sqlx::query_as(
        "SELECT user_id, status FROM mpc_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_optional(db)
    .await
    .map_err(|e| {
        tracing::error!("Failed to query session: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    match session_user {
        Some((session_user_id, status)) => {
            if session_user_id != user_id {
                tracing::warn!(
                    "User {} attempted to access backup contribution for session {} owned by {}",
                    user_id, session_id, session_user_id
                );
                return Err(StatusCode::FORBIDDEN);
            }

            // Only allow fetching for completed DKG sessions
            if status != "completed" {
                tracing::warn!(
                    "Backup contribution requested for session {} with status '{}'",
                    session_id, status
                );
                return Err(StatusCode::CONFLICT);
            }
        }
        None => {
            return Err(StatusCode::NOT_FOUND);
        }
    }

    // Fetch the backup contribution from the MPC participant
    if let Some(participant) = &state.mpc_participant {
        if let Some(contribution) = participant.fetch_backup_contribution(session_id, user_id) {
            if contribution.len() != 32 {
                tracing::error!(
                    "Invalid backup contribution length for session {}: {} bytes",
                    session_id, contribution.len()
                );
                return Err(StatusCode::INTERNAL_SERVER_ERROR);
            }

            tracing::info!(
                "User {} fetched backup contribution for session {}",
                user_id, session_id
            );
            return Ok(Json(contribution));
        }
    }

    // Contribution not available (either never computed, already fetched, or expired)
    tracing::debug!(
        "Backup contribution not available for session {} (user {})",
        session_id, user_id
    );
    Err(StatusCode::NOT_FOUND)
}

// ─── Presignature Management Endpoints ───────────────────────────────────────

#[derive(Deserialize)]
pub(crate) struct PresignStatusQuery {
    wallet_id: Option<uuid::Uuid>,
    address: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct PresignStatusResponse {
    available: i64,
    wallet_id: String,
}

/// GET /presign/status?wallet_id={uuid} or ?address={0x...}
/// Returns the number of available presignatures for the given wallet.
pub async fn presign_status(
    State(state): State<AppState>,
    Extension(_claims): Extension<Claims>,
    Query(query): Query<PresignStatusQuery>,
) -> Result<Json<PresignStatusResponse>, StatusCode> {
    let presign_mgr = state.presign_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let wallet_id = if let Some(id) = query.wallet_id {
        id
    } else if let Some(addr) = &query.address {
        let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        let addr_bytes = hex::decode(addr.trim_start_matches("0x"))
            .map_err(|_| StatusCode::BAD_REQUEST)?;
        let row: Option<(uuid::Uuid,)> = sqlx::query_as(
            "SELECT id FROM wallets WHERE eth_address = $1"
        )
        .bind(addr_bytes)
        .fetch_optional(db)
        .await
        .map_err(|e| {
            tracing::error!("presign_status wallet lookup error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        row.ok_or(StatusCode::NOT_FOUND)?.0
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };

    let available = presign_mgr
        .get_available_count(wallet_id)
        .await
        .map_err(|e| {
            tracing::error!("presign_status error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(PresignStatusResponse {
        available,
        wallet_id: wallet_id.to_string(),
    }))
}

#[derive(Deserialize)]
pub(crate) struct PresignGenerateRequest {
    wallet_id: uuid::Uuid,
    count: Option<u32>,
}

#[derive(Serialize)]
pub(crate) struct PresignGenerateResponse {
    generated: u32,
    wallet_id: String,
}

/// POST /presign/generate
/// Triggers presignature generation for the given wallet.
pub async fn presign_generate(
    State(state): State<AppState>,
    Extension(claims): Extension<Claims>,
    Json(body): Json<PresignGenerateRequest>,
) -> Result<Json<PresignGenerateResponse>, StatusCode> {
    let presign_mgr = state.presign_manager
        .as_ref()
        .ok_or(StatusCode::SERVICE_UNAVAILABLE)?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    let count = body.count.unwrap_or(5).min(50);

    let generated = presign_mgr
        .generate_presignatures(user_id, body.wallet_id, count)
        .await
        .map_err(|e| {
            tracing::error!("presign_generate error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(
        "User {} generated {} presignatures for wallet {}",
        user_id, generated, body.wallet_id
    );

    Ok(Json(PresignGenerateResponse {
        generated,
        wallet_id: body.wallet_id.to_string(),
    }))
}
