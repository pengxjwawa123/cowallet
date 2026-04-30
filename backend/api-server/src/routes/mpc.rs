use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{delete, get, post},
    Extension,
};
use serde::{Deserialize, Serialize};

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
struct CreateSessionRequest {
    session_type: String,
    parties: Vec<i16>,
    threshold: Option<i16>,
}

#[derive(Serialize)]
struct SessionResponse {
    session_id: String,
    status: String,
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

    // 从 JWT claims 取真实 user_id
    let initiator_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // 统一 session_type：客户端发 'keygen' 映射到数据库的 'dkg'
    let db_session_type = match body.session_type.as_str() {
        "keygen" => "dkg",
        other => other,
    };

    sqlx::query(
        "INSERT INTO mpc_sessions (id, session_type, initiator_id, parties, threshold, total_parties)
         VALUES ($1, $2, $3, $4, $5, $6)"
    )
    .bind(session_id)
    .bind(db_session_type)
    .bind(initiator_id)
    .bind(&body.parties)
    .bind(threshold)
    .bind(total_parties)
    .execute(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(SessionResponse {
        session_id: session_id.to_string(),
        status: "pending".into(),
    }))
}

async fn get_session(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<Json<SessionResponse>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let row: (String,) = sqlx::query_as("SELECT status FROM mpc_sessions WHERE id = $1")
        .bind(id)
        .fetch_one(db)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    Ok(Json(SessionResponse {
        session_id: id.to_string(),
        status: row.0,
    }))
}

async fn abort_session(
    State(state): State<AppState>,
    Path(id): Path<uuid::Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    sqlx::query(
        "UPDATE mpc_sessions SET status = 'failed' WHERE id = $1 AND status IN ('pending', 'active')"
    )
    .bind(id)
    .execute(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct SendMessageRequest {
    from_party: i16,
    to_party: i16,
    round: i16,
    payload: Vec<u8>,
}

async fn send_message(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
    Json(body): Json<SendMessageRequest>,
) -> Result<StatusCode, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    sqlx::query(
        "INSERT INTO mpc_messages (session_id, from_party, to_party, round, payload)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(session_id)
    .bind(body.from_party)
    .bind(body.to_party)
    .bind(body.round)
    .bind(&body.payload)
    .execute(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::CREATED)
}

#[derive(Serialize)]
struct MessageResponse {
    from_party: i16,
    round: i16,
    payload: Vec<u8>,
}

async fn recv_messages(
    State(state): State<AppState>,
    Path(session_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<MessageResponse>>, StatusCode> {
    let db = state
        .require_db()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let rows: Vec<(i16, i16, Vec<u8>)> = sqlx::query_as(
        "SELECT from_party, round, payload FROM mpc_messages
         WHERE session_id = $1
         ORDER BY round, created_at",
    )
    .bind(session_id)
    .fetch_all(db)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let messages: Vec<MessageResponse> = rows
        .into_iter()
        .map(|(from_party, round, payload)| MessageResponse {
            from_party,
            round,
            payload,
        })
        .collect();

    Ok(Json(messages))
}
