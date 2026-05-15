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

use mpc_core::transport::noise::NoiseSession;

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
/// Payload is base64-encoded for compact transport over Noise-encrypted WebSocket.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub from_party: i16,
    pub to_party: i16,
    pub round: i16,
    #[serde(with = "payload_b64")]
    pub payload: Vec<u8>,
}

/// Serde module: serializes Vec<u8> as base64 string, deserializes from either
/// base64 string or JSON int array (backwards compatible with old clients).
mod payload_b64 {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
    use serde::{self, Deserialize, Deserializer, Serializer, de};

    pub fn serialize<S>(bytes: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        serializer.serialize_str(&BASE64.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where D: Deserializer<'de> {
        let value = serde_json::Value::deserialize(deserializer)?;
        match value {
            serde_json::Value::String(s) => {
                BASE64.decode(&s).map_err(de::Error::custom)
            }
            serde_json::Value::Array(arr) => {
                arr.iter()
                    .map(|v| v.as_u64().ok_or_else(|| de::Error::custom("invalid byte")).and_then(|n| {
                        u8::try_from(n).map_err(|_| de::Error::custom("byte out of range"))
                    }))
                    .collect()
            }
            _ => Err(de::Error::custom("payload must be base64 string or byte array")),
        }
    }
}

/// Noise_XX handshake control messages exchanged before encrypted transport begins.
/// The client initiates by sending a `NoiseHandshake` message with step=1.
/// The server responds, and one more round-trip completes the XX pattern.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseHandshakeMsg {
    /// Discriminator: always "noise_handshake"
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Handshake step number (1, 2, or 3)
    pub step: u8,
    /// Base64-encoded handshake payload
    pub data: String,
}

/// Envelope for Noise-encrypted MPC messages after handshake completes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoiseEncryptedMsg {
    /// Discriminator: always "noise_encrypted"
    #[serde(rename = "type")]
    pub msg_type: String,
    /// Base64-encoded ciphertext (decrypts to JSON WsMessage)
    pub data: String,
}

/// Checks if a JSON object has a "type" field matching the given value.
fn json_has_type(text: &str, type_value: &str) -> bool {
    // Quick check without full deserialization
    text.contains(&format!("\"type\":\"{}\"", type_value))
        || text.contains(&format!("\"type\": \"{}\"", type_value))
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
///
/// Accepts plain JSON messages directly (no Noise handshake required).
async fn handle_ws_connection(
    socket: WebSocket,
    state: AppState,
    session_id: Uuid,
    party_index: i16,
) {
    let (mut ws_sink, mut ws_stream) = socket.split();

    tracing::info!(
        "WS connected: session {} party {} (plain JSON transport)",
        session_id, party_index
    );

    // No Noise session — plain JSON transport
    let noise = std::sync::Arc::new(tokio::sync::Mutex::new(None::<NoiseSession>));

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
                let send_result = send_ws_message(&mut ws_sink, &noise, &ws_msg).await;
                if send_result.is_err() {
                    return; // Client disconnected during catch-up
                }
            }
        }
    }

    // Spawn task: forward messages from channel to WS sink
    let noise_sink = noise.clone();
    let sink_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if send_ws_message(&mut ws_sink, &noise_sink, &msg).await.is_err() {
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
                let ws_msg = recv_ws_text_message(&text, &noise).await;
                match ws_msg {
                    Ok(Some(m)) => handle_client_message(&state, session_id, party_index, m).await,
                    Ok(None) => {} // non-MPC control message (e.g. ping)
                    Err(e) => {
                        tracing::warn!(
                            "WS message processing failed for session {} party {}: {}",
                            session_id, party_index, e
                        );
                    }
                }
            }
            Message::Binary(data) => {
                let text = String::from_utf8_lossy(&data);
                let ws_msg = recv_ws_text_message(&text, &noise).await;
                match ws_msg {
                    Ok(Some(m)) => handle_client_message(&state, session_id, party_index, m).await,
                    Ok(None) => {}
                    Err(e) => {
                        tracing::warn!(
                            "WS binary processing failed for session {} party {}: {} (len: {})",
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

/// Perform the server side of a Noise_XX handshake over WebSocket.
///
/// The client has already sent step 1 (-> e). This function:
/// 1. Parses step 1 from the client
/// 2. Generates the server's response (step 2: <- e, ee, s, es)
/// 3. Receives the client's final message (step 3: -> s, se)
/// 4. Returns the completed NoiseSession ready for transport
#[allow(dead_code)]
async fn perform_noise_handshake(
    first_msg_text: &str,
    ws_sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    ws_stream: &mut futures::stream::SplitStream<WebSocket>,
    state: &AppState,
) -> std::result::Result<NoiseSession, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    // Parse the client's step 1 message
    let hs_msg: NoiseHandshakeMsg = serde_json::from_str(first_msg_text)
        .map_err(|e| format!("invalid noise_handshake message: {}", e))?;

    if hs_msg.step != 1 {
        return Err(format!("expected handshake step 1, got {}", hs_msg.step));
    }

    let client_msg1 = BASE64.decode(&hs_msg.data)
        .map_err(|e| format!("invalid base64 in handshake step 1: {}", e))?;

    // Get or generate server's static key
    let server_static_key = get_server_noise_key(state);

    // Create responder session
    let mut session = NoiseSession::new_responder(&server_static_key)
        .map_err(|e| format!("failed to create noise responder: {}", e))?;

    // Process client's msg1 (-> e) and generate server response (<- e, ee, s, es)
    let server_msg2 = session.handshake_step(&client_msg1)
        .map_err(|e| format!("noise handshake step 2 failed: {}", e))?;

    // Send step 2 to client
    let step2_response = serde_json::json!({
        "type": "noise_handshake",
        "step": 2,
        "data": BASE64.encode(&server_msg2),
    });
    ws_sink
        .send(Message::Text(step2_response.to_string().into()))
        .await
        .map_err(|e| format!("failed to send handshake step 2: {}", e))?;

    // Receive step 3 from client (-> s, se)
    let step3_msg = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        ws_stream.next(),
    )
    .await
    .map_err(|_| "noise handshake step 3 timed out".to_string())?
    .ok_or_else(|| "client disconnected during noise handshake".to_string())?
    .map_err(|e| format!("ws error during handshake step 3: {}", e))?;

    let step3_text = match step3_msg {
        Message::Text(t) => t.to_string(),
        Message::Binary(d) => String::from_utf8(d.to_vec())
            .map_err(|_| "invalid UTF-8 in handshake step 3".to_string())?,
        _ => return Err("unexpected message type during handshake step 3".to_string()),
    };

    let hs_msg3: NoiseHandshakeMsg = serde_json::from_str(&step3_text)
        .map_err(|e| format!("invalid noise_handshake step 3: {}", e))?;

    if hs_msg3.step != 3 {
        return Err(format!("expected handshake step 3, got {}", hs_msg3.step));
    }

    let client_msg3 = BASE64.decode(&hs_msg3.data)
        .map_err(|e| format!("invalid base64 in handshake step 3: {}", e))?;

    // Process final handshake message — session transitions to transport mode
    let _empty = session.handshake_step(&client_msg3)
        .map_err(|e| format!("noise handshake finalization failed: {}", e))?;

    if !session.is_transport_ready() {
        return Err("noise session not in transport mode after handshake".to_string());
    }

    Ok(session)
}

/// Get the server's Noise static private key.
/// In production this should come from an HSM or env var.
/// Falls back to a deterministic key derived from the encryption key for consistency.
#[allow(dead_code)]
fn get_server_noise_key(state: &AppState) -> Vec<u8> {
    // Derive from NOISE_STATIC_KEY env var if set, otherwise generate deterministically
    // from the server's encryption key using HKDF.
    if let Ok(key_hex) = std::env::var("NOISE_STATIC_KEY") {
        if let Ok(key_bytes) = hex::decode(&key_hex) {
            if key_bytes.len() == 32 {
                return key_bytes;
            }
        }
        tracing::warn!("NOISE_STATIC_KEY env var invalid, falling back to derived key");
    }

    // Derive a stable key from the server's encryption key using HKDF-SHA256
    use hkdf::Hkdf;
    use sha2::Sha256;

    let ikm = std::env::var("ENCRYPTION_KEY")
        .unwrap_or_else(|_| "cowallet-noise-default-ikm-not-for-production".to_string());
    let hkdf = Hkdf::<Sha256>::new(Some(b"cowallet-noise-xx-static-key"), ikm.as_bytes());
    let mut key = vec![0u8; 32];
    hkdf.expand(b"noise_xx_server_static", &mut key)
        .expect("32 bytes is valid for HKDF-SHA256");
    key
}

/// Send a WsMessage over the WebSocket, encrypting with Noise if a session is active.
async fn send_ws_message(
    ws_sink: &mut futures::stream::SplitSink<WebSocket, Message>,
    noise: &std::sync::Arc<tokio::sync::Mutex<Option<NoiseSession>>>,
    ws_msg: &WsMessage,
) -> std::result::Result<(), String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    let json = serde_json::to_string(ws_msg)
        .map_err(|e| format!("serialization failed: {}", e))?;

    let mut noise_guard = noise.lock().await;
    let send_text = if let Some(ref mut session) = *noise_guard {
        // Encrypt and wrap
        let ciphertext = session.encrypt(json.as_bytes())
            .map_err(|e| format!("noise encryption failed: {}", e))?;
        serde_json::json!({
            "type": "noise_encrypted",
            "data": BASE64.encode(&ciphertext),
        }).to_string()
    } else {
        json
    };
    drop(noise_guard);

    ws_sink
        .send(Message::Text(send_text.into()))
        .await
        .map_err(|e| format!("ws send failed: {}", e))
}

/// Process an incoming WebSocket text message, decrypting with Noise if active.
/// Returns `Ok(Some(msg))` for MPC messages, `Ok(None)` for control messages.
async fn recv_ws_text_message(
    text: &str,
    noise: &std::sync::Arc<tokio::sync::Mutex<Option<NoiseSession>>>,
) -> std::result::Result<Option<WsMessage>, String> {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

    // Check if it's a Noise-encrypted message
    if json_has_type(text, "noise_encrypted") {
        let enc_msg: NoiseEncryptedMsg = serde_json::from_str(text)
            .map_err(|e| format!("invalid noise_encrypted message: {}", e))?;

        let ciphertext = BASE64.decode(&enc_msg.data)
            .map_err(|e| format!("invalid base64 in noise_encrypted: {}", e))?;

        let mut noise_guard = noise.lock().await;
        let session = noise_guard.as_mut()
            .ok_or_else(|| "received noise_encrypted but no noise session active".to_string())?;

        let plaintext = session.decrypt(&ciphertext)
            .map_err(|e| format!("noise decryption failed: {}", e))?;
        drop(noise_guard);

        let ws_msg: WsMessage = serde_json::from_slice(&plaintext)
            .map_err(|e| format!("decrypted payload is not valid WsMessage: {}", e))?;

        return Ok(Some(ws_msg));
    }

    // Plain (unencrypted) message — parse directly
    match serde_json::from_str::<WsMessage>(text) {
        Ok(ws_msg) => Ok(Some(ws_msg)),
        Err(_) => {
            // Might be a control message (ping/pong JSON)
            Ok(None)
        }
    }
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
        let nats_msg = serde_json::json!({
            "session_id": session_id.to_string(),
            "from_party": ws_msg.from_party,
            "to_party": ws_msg.to_party,
            "round": ws_msg.round,
            "payload": ws_msg.payload,
            "timestamp": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        });
        let payload = match serde_json::to_vec(&nats_msg) {
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
                        if let Some(nats) = &state.nats {
                            let subject = format!("cowallet.mpc.{}.{}", session_id, target_party);
                            let nats_msg = serde_json::json!({
                                "session_id": session_id.to_string(),
                                "from_party": from,
                                "to_party": target_party,
                                "round": ws_msg.round + 1,
                                "payload": payload,
                                "timestamp": std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs(),
                            });
                            if let Ok(data) = serde_json::to_vec(&nats_msg) {
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
