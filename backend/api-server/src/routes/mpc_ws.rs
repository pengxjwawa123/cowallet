use axum::{
    Router,
    extract::{Path, Query, State, WebSocketUpgrade, ws::{Message, WebSocket}},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::middleware::auth::verify_token_unchecked;
use crate::state::AppState;

/// Query parameters for the WebSocket upgrade request.
#[derive(Deserialize)]
pub struct WsQuery {
    /// The party index this client represents.
    party: i16,
    /// JWT token for authentication (passed as query param since WS doesn't support headers).
    token: String,
}

/// JSON message format sent over the WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub from_party: i16,
    pub to_party: i16,
    pub round: i16,
    pub payload: Vec<u8>,
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/mpc/session/{id}/ws", get(ws_handler))
}

/// Handle WebSocket upgrade: validate JWT and party membership, then upgrade.
async fn ws_handler(
    State(state): State<AppState>,
    Path(session_id): Path<Uuid>,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, StatusCode> {
    // Validate JWT from query parameter
    let claims = verify_token_unchecked(&query.token)
        .map_err(|_| StatusCode::UNAUTHORIZED)?;

    // Verify the session exists and the party belongs to it
    let db = state.require_db().map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    let session: (String, Vec<i16>, Uuid) = sqlx::query_as(
        "SELECT status, parties, user_id FROM mpc_sessions WHERE id = $1"
    )
    .bind(session_id)
    .fetch_one(db)
    .await
    .map_err(|_| StatusCode::NOT_FOUND)?;

    let (status, parties, session_user_id) = session;

    // Session must be active
    if status != "active" {
        return Err(StatusCode::GONE);
    }

    // Verify party index is part of this session
    if !parties.contains(&query.party) {
        return Err(StatusCode::FORBIDDEN);
    }

    // Verify the authenticated user owns this session
    let user_id = Uuid::parse_str(&claims.sub).map_err(|_| StatusCode::UNAUTHORIZED)?;
    if user_id != session_user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    let party_index = query.party;

    Ok(ws.on_upgrade(move |socket| {
        handle_ws_connection(socket, state, session_id, party_index)
    }))
}

/// Main WebSocket connection handler.
/// Subscribes to NATS for real-time push, or falls back to DB polling.
/// Forwards incoming WS messages to the appropriate party via NATS or DB.
async fn handle_ws_connection(
    socket: WebSocket,
    state: AppState,
    session_id: Uuid,
    party_index: i16,
) {
    let (mut ws_sink, mut ws_stream) = socket.split();

    // Channel for pushing messages to the WS client
    let (tx, mut rx) = mpsc::channel::<WsMessage>(64);

    // Send any messages that arrived before this WS connection (catch-up from DB)
    if let Some(db) = &state.db {
        let catchup_messages: Result<Vec<(i64, i16, i16, i16, Vec<u8>)>, _> = sqlx::query_as(
            "SELECT id, from_party, to_party, round, payload
             FROM mpc_messages
             WHERE session_id = $1 AND (to_party = $2 OR to_party = -1)
             ORDER BY round ASC, created_at ASC"
        )
        .bind(session_id)
        .bind(party_index)
        .fetch_all(db)
        .await;

        if let Ok(messages) = catchup_messages {
            for msg in messages {
                let ws_msg = WsMessage {
                    from_party: msg.1,
                    to_party: msg.2,
                    round: msg.3,
                    payload: msg.4,
                };
                if let Ok(json) = serde_json::to_string(&ws_msg) {
                    if ws_sink.send(Message::Text(json.into())).await.is_err() {
                        return; // Client disconnected during catch-up
                    }
                }
            }
        }
    }

    // Spawn task: forward messages from channel to WS sink
    let sink_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            let json = match serde_json::to_string(&msg) {
                Ok(j) => j,
                Err(_) => continue,
            };
            if ws_sink.send(Message::Text(json.into())).await.is_err() {
                break; // Client disconnected
            }
        }
    });

    // Spawn task: subscribe to NATS or poll DB for new messages destined to this party
    let tx_clone = tx.clone();
    let state_clone = state.clone();
    let inbound_task = tokio::spawn(async move {
        let nats_subject = format!("cowallet.mpc.{}.{}", session_id, party_index);

        if let Some(nats) = &state_clone.nats {
            // NATS-based real-time subscription
            match nats.subscribe(nats_subject.clone()).await {
                Ok(mut subscriber) => {
                    tracing::debug!(
                        "WS party {} subscribed to NATS subject: {}",
                        party_index, nats_subject
                    );
                    while let Some(nats_msg) = subscriber.next().await {
                        if let Ok(ws_msg) = serde_json::from_slice::<WsMessage>(&nats_msg.payload) {
                            if tx_clone.send(ws_msg).await.is_err() {
                                break; // Channel closed, WS disconnected
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "NATS subscribe failed for {}: {} — falling back to DB polling",
                        nats_subject, e
                    );
                    // Fall through to DB polling
                    db_poll_loop(&state_clone, session_id, party_index, &tx_clone).await;
                }
            }
        } else {
            // No NATS available: fall back to DB polling
            db_poll_loop(&state_clone, session_id, party_index, &tx_clone).await;
        }
    });

    // Handle incoming WS messages from the client (client sending MPC messages)
    while let Some(result) = ws_stream.next().await {
        let msg = match result {
            Ok(msg) => msg,
            Err(e) => {
                tracing::debug!("WS recv error for session {} party {}: {}", session_id, party_index, e);
                break;
            }
        };

        match msg {
            Message::Text(text) => {
                match serde_json::from_str::<WsMessage>(&text) {
                    Ok(ws_msg) => {
                        handle_client_message(&state, session_id, party_index, ws_msg).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "WS text deserialization failed for session {} party {}: {} (first 200 chars: {})",
                            session_id, party_index, e,
                            &text[..text.len().min(200)]
                        );
                    }
                }
            }
            Message::Binary(data) => {
                match serde_json::from_slice::<WsMessage>(&data) {
                    Ok(ws_msg) => {
                        handle_client_message(&state, session_id, party_index, ws_msg).await;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "WS binary deserialization failed for session {} party {}: {} (len: {})",
                            session_id, party_index, e, data.len()
                        );
                    }
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) | Message::Pong(_) => {} // handled by axum automatically
        }
    }

    // Client disconnected: clean up
    tracing::info!("WS disconnected: session {} party {}", session_id, party_index);
    sink_task.abort();
    inbound_task.abort();
}

/// Handle an MPC message received from the WS client.
/// Store it in DB and publish to NATS (if available) for the target party.
/// If the message is addressed to the server (Party 1), trigger the MPC participant.
async fn handle_client_message(
    state: &AppState,
    session_id: Uuid,
    from_party: i16,
    ws_msg: WsMessage,
) {
    tracing::info!(
        "WS received: session {} from_party={} to_party={} round={} payload_len={}",
        session_id, ws_msg.from_party, ws_msg.to_party, ws_msg.round, ws_msg.payload.len()
    );

    // Validate that from_party matches the authenticated party
    if ws_msg.from_party != from_party {
        tracing::warn!(
            "WS message from_party mismatch: claimed {} but authenticated as {}",
            ws_msg.from_party, from_party
        );
        return;
    }

    // Store the message in DB
    if let Some(db) = &state.db {
        let store_result = sqlx::query(
            "INSERT INTO mpc_messages (session_id, from_party, to_party, round, payload, verified)
             VALUES ($1, $2, $3, $4, $5, false)"
        )
        .bind(session_id)
        .bind(ws_msg.from_party)
        .bind(ws_msg.to_party)
        .bind(ws_msg.round)
        .bind(&ws_msg.payload)
        .execute(db)
        .await;

        if let Err(e) = store_result {
            tracing::error!("Failed to store WS message in DB: {}", e);
            return;
        }

        // Update session activity
        let _ = sqlx::query(
            "UPDATE mpc_sessions SET last_activity = NOW(), current_round = GREATEST(current_round, $2)
             WHERE id = $1"
        )
        .bind(session_id)
        .bind(ws_msg.round as i32)
        .execute(db)
        .await;
    }

    // Publish to NATS for the target party (real-time push)
    if let Some(nats) = &state.nats {
        let target_subject = format!("cowallet.mpc.{}.{}", session_id, ws_msg.to_party);
        let payload = match serde_json::to_vec(&ws_msg) {
            Ok(p) => p,
            Err(_) => return,
        };
        if let Err(e) = nats.publish(target_subject.clone(), payload.into()).await {
            tracing::warn!("NATS publish to {} failed: {}", target_subject, e);
        }
    }

    // If addressed to server (Party 1), trigger the MPC participant
    if ws_msg.to_party == 1 {
        if let Some(participant) = &state.mpc_participant {
            match participant.on_message_received(
                session_id,
                ws_msg.from_party,
                ws_msg.round,
                &ws_msg.payload,
            ).await {
                Ok(responses) => {
                    // Publish server responses via NATS for the requesting party
                    for (from, to, payload) in responses {
                        // to == -1 means broadcast; deliver to the requesting party
                        let target_party = if to == -1 { ws_msg.from_party } else { to };
                        let response_msg = WsMessage {
                            from_party: from,
                            to_party: target_party,
                            round: ws_msg.round + 1,
                            payload,
                        };
                        if let Some(nats) = &state.nats {
                            let subject = format!("cowallet.mpc.{}.{}", session_id, target_party);
                            if let Ok(data) = serde_json::to_vec(&response_msg) {
                                let _ = nats.publish(subject, data.into()).await;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "MPC participant error for session {} round {}: {}",
                        session_id, ws_msg.round, e
                    );
                }
            }
        }
    }
}

/// DB polling fallback: check for new messages every 200ms.
/// Used when NATS is not available or subscription fails.
async fn db_poll_loop(
    state: &AppState,
    session_id: Uuid,
    party_index: i16,
    tx: &mpsc::Sender<WsMessage>,
) {
    let mut last_id: i64 = 0;
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(200));

    loop {
        interval.tick().await;

        let db = match &state.db {
            Some(db) => db,
            None => continue,
        };

        let messages: Result<Vec<(i64, i16, i16, i16, Vec<u8>)>, _> = sqlx::query_as(
            "SELECT id, from_party, to_party, round, payload
             FROM mpc_messages
             WHERE session_id = $1 AND id > $2 AND (to_party = $3 OR to_party = -1)
             ORDER BY id ASC
             LIMIT 50"
        )
        .bind(session_id)
        .bind(last_id)
        .bind(party_index)
        .fetch_all(db)
        .await;

        match messages {
            Ok(rows) => {
                for row in rows {
                    last_id = row.0;
                    let ws_msg = WsMessage {
                        from_party: row.1,
                        to_party: row.2,
                        round: row.3,
                        payload: row.4,
                    };
                    if tx.send(ws_msg).await.is_err() {
                        return; // Channel closed
                    }
                }
            }
            Err(e) => {
                tracing::warn!("DB poll error for session {} party {}: {}", session_id, party_index, e);
            }
        }

        // Check if session is still active
        let status: Result<Option<String>, _> = sqlx::query_scalar(
            "SELECT status FROM mpc_sessions WHERE id = $1"
        )
        .bind(session_id)
        .fetch_optional(db)
        .await;

        match status {
            Ok(Some(s)) if s == "active" => {} // continue polling
            _ => return, // session ended or error
        }
    }
}
