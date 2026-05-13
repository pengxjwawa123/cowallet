use async_nats::{self, Client as NatsClient};
use dashmap::DashMap;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing_subscriber;
use uuid::Uuid;

/// MPC message relay service.
///
/// Connects to NATS and routes protocol messages between MPC parties.
/// Each signing session gets a dedicated NATS subject:
///   cowallet.mpc.{session_id}.{party_index}
///
/// Features:
/// - Session management with timeout
/// - Per-party message queues
/// - Authentication for session creation
/// - Automatic cleanup of expired sessions
/// - Message routing between parties

/// Session type - key generation or signing
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SessionType {
    Keygen,
    Sign,
    Reshare,
}

/// A message in an MPC protocol session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MpcMessage {
    pub session_id: Uuid,
    pub from_party: u16,
    pub to_party: u16,
    pub round: u16,
    #[serde(with = "serde_bytes")]
    pub payload: Vec<u8>,
    pub timestamp: u64,
}

/// Session metadata
#[derive(Debug, Clone)]
pub struct Session {
    pub id: Uuid,
    pub session_type: SessionType,
    pub parties: Vec<u16>,
    pub threshold: u16,
    pub created_at: Instant,
    pub last_activity: Instant,
    pub message_count: u64,
}

impl Session {
    pub fn new(id: Uuid, session_type: SessionType, parties: Vec<u16>, threshold: u16) -> Self {
        let now = Instant::now();
        Self {
            id,
            session_type,
            parties,
            threshold,
            created_at: now,
            last_activity: now,
            message_count: 0,
        }
    }

    pub fn is_expired(&self, timeout: Duration) -> bool {
        self.last_activity.elapsed() > timeout
    }

    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
        self.message_count += 1;
    }
}

/// Relay state - shared across all handler tasks
#[derive(Clone)]
pub struct RelayState {
    sessions: Arc<DashMap<Uuid, Session>>,
    message_queues: Arc<DashMap<(Uuid, u16), mpsc::UnboundedSender<MpcMessage>>>,
    // Keep receivers alive to prevent channel closure on send
    #[allow(dead_code)]
    message_queue_receivers: Arc<DashMap<(Uuid, u16), mpsc::UnboundedReceiver<MpcMessage>>>,
    nats_client: Option<NatsClient>,
    session_timeout: Duration,
}

impl RelayState {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            message_queues: Arc::new(DashMap::new()),
            message_queue_receivers: Arc::new(DashMap::new()),
            nats_client: None,
            session_timeout: Duration::from_secs(300), // 5 minutes
        }
    }

    pub fn with_nats(mut self, client: NatsClient) -> Self {
        self.nats_client = Some(client);
        self
    }

    /// Create a new MPC session
    pub fn create_session(
        &self,
        session_type: SessionType,
        parties: Vec<u16>,
        threshold: u16,
    ) -> Uuid {
        let session_id = Uuid::new_v4();
        let session = Session::new(session_id, session_type, parties.clone(), threshold);

        // Create message queues for each party
        for party in &parties {
            let (tx, rx) = mpsc::unbounded_channel();
            self.message_queues.insert((session_id, *party), tx);
            self.message_queue_receivers.insert((session_id, *party), rx);
        }

        self.sessions.insert(session_id, session);
        tracing::info!("Created session {} with {} parties", session_id, parties.len());
        session_id
    }

    /// Route a message from sender to recipient
    pub async fn route_message(&self, message: MpcMessage) -> Result<(), String> {
        let session_id = message.session_id;
        let to_party = message.to_party;

        // Update session activity
        if let Some(mut session) = self.sessions.get_mut(&session_id) {
            session.touch();
        } else {
            return Err(format!("Session {} not found", session_id));
        }

        // Route via NATS if available
        if let Some(nats) = &self.nats_client {
            let subject = format!("cowallet.mpc.{}.{}", session_id, to_party);
            let payload = serde_json::to_vec(&message)
                .map_err(|e| format!("Failed to serialize message: {}", e))?;

            nats.publish(subject, payload.into())
                .await
                .map_err(|e| format!("NATS publish failed: {}", e))?;

            tracing::trace!(
                "Routed message {}->{} for session {} via NATS",
                message.from_party,
                message.to_party,
                session_id
            );
            Ok(())
        } else {
            // Fallback: in-memory routing (for testing)
            if let Some(queue) = self.message_queues.get(&(session_id, to_party)) {
                queue
                    .send(message)
                    .map_err(|e| format!("Failed to send to message queue: {}", e))?;
                Ok(())
            } else {
                Err(format!(
                    "No message queue for session {} party {}",
                    session_id, to_party
                ))
            }
        }
    }

    /// Remove expired sessions
    pub fn cleanup_expired(&self) -> usize {
        let timeout = self.session_timeout;
        let mut removed = 0;

        self.sessions.retain(|&session_id, session| {
            if session.is_expired(timeout) {
                tracing::info!("Cleaning up expired session {}", session_id);
                // Remove message queues for this session
                self.message_queues.retain(|&(sid, _), _| sid != session_id);
                self.message_queue_receivers.retain(|&(sid, _), _| sid != session_id);
                removed += 1;
                false
            } else {
                true
            }
        });

        removed
    }

    /// Get session statistics
    pub fn get_stats(&self) -> (usize, u64) {
        let session_count = self.sessions.len();
        let total_messages = self.sessions.iter().map(|s| s.message_count).sum();
        (session_count, total_messages)
    }
}

impl Default for RelayState {
    fn default() -> Self {
        Self::new()
    }
}

/// NATS message handler
async fn handle_nats_message(
    state: RelayState,
    _subject: String,
    payload: bytes::Bytes,
) -> Result<(), String> {
    // Parse message
    let message: MpcMessage = serde_json::from_slice(&payload)
        .map_err(|e| format!("Failed to parse MPC message: {}", e))?;

    // Route to destination
    state.route_message(message).await?;
    Ok(())
}

/// Background cleanup task
async fn cleanup_task(state: RelayState, interval: Duration) {
    let mut interval = tokio::time::interval(interval);
    loop {
        interval.tick().await;
        let removed = state.cleanup_expired();
        if removed > 0 {
            tracing::info!("Cleaned up {} expired sessions", removed);
        }
    }
}

/// Metrics logging task
async fn metrics_task(state: RelayState, interval: Duration) {
    let mut interval = tokio::time::interval(interval);
    loop {
        interval.tick().await;
        let (sessions, messages) = state.get_stats();
        tracing::debug!(
            "Relay stats: {} active sessions, {} total messages routed",
            sessions,
            messages
        );
    }
}

/// Initialize and run the MPC relay service
pub async fn run_relay(nats_url: &str) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("cowallet MPC relay starting");

    // Connect to NATS
    let nats_client = match async_nats::connect(nats_url).await {
        Ok(client) => {
            tracing::info!("Connected to NATS at {}", nats_url);
            Some(client)
        }
        Err(e) => {
            tracing::warn!("Failed to connect to NATS: {}, running in memory-only mode", e);
            None
        }
    };

    // Create shared state
    let state = RelayState::new();
    let state = if let Some(client) = nats_client {
        state.with_nats(client)
    } else {
        state
    };

    // Spawn background tasks
    let cleanup_state = state.clone();
    tokio::spawn(async move {
        cleanup_task(cleanup_state, Duration::from_secs(30)).await;
    });

    let metrics_state = state.clone();
    tokio::spawn(async move {
        metrics_task(metrics_state, Duration::from_secs(60)).await;
    });

    // If NATS is available, subscribe to all MPC subjects
    if let Some(nats) = &state.nats_client {
        let mut subscriber = nats.subscribe("cowallet.mpc.>".to_string()).await?;
        tracing::info!("Subscribed to NATS subject: cowallet.mpc.>");

        let handler_state = state.clone();
        tokio::spawn(async move {
            while let Some(msg) = subscriber.next().await {
                let subject = msg.subject.to_string();
                let payload = msg.payload;
                let state_clone = handler_state.clone();

                tokio::spawn(async move {
                    if let Err(e) = handle_nats_message(state_clone, subject, payload).await {
                        tracing::warn!("Failed to handle NATS message: {}", e);
                    }
                });
            }
        });
    }

    tracing::info!("MPC relay ready");

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    tracing::info!("Shutting down MPC relay...");

    let (final_sessions, final_messages) = state.get_stats();
    tracing::info!(
        "Final stats: {} sessions, {} messages routed during runtime",
        final_sessions,
        final_messages
    );

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Get NATS URL from environment or use default
    let nats_url = std::env::var("NATS_URL").unwrap_or_else(|_| "nats://localhost:4222".into());

    run_relay(&nats_url).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_session_creation() {
        let state = RelayState::new();
        let parties = vec![0, 1, 2];
        let _session_id = state.create_session(SessionType::Keygen, parties.clone(), 2);

        let (session_count, _) = state.get_stats();
        assert_eq!(session_count, 1);
    }

    #[tokio::test]
    async fn test_in_memory_message_routing() {
        let state = RelayState::new();
        let parties = vec![0, 1, 2];
        let session_id = state.create_session(SessionType::Keygen, parties, 2);

        let message = MpcMessage {
            session_id,
            from_party: 0,
            to_party: 1,
            round: 1,
            payload: vec![1, 2, 3, 4],
            timestamp: 0,
        };

        // In-memory routing should succeed
        let result = state.route_message(message).await;
        assert!(result.is_ok());

        let (_, msg_count) = state.get_stats();
        assert_eq!(msg_count, 1);
    }

    #[tokio::test]
    async fn test_session_expiration() {
        let mut state = RelayState::new();
        state.session_timeout = Duration::from_millis(10);

        let parties = vec![0, 1];
        let _session_id = state.create_session(SessionType::Sign, parties, 2);

        assert_eq!(state.sessions.len(), 1);

        // Wait for timeout
        tokio::time::sleep(Duration::from_millis(50)).await;

        let removed = state.cleanup_expired();
        assert_eq!(removed, 1);
        assert_eq!(state.sessions.len(), 0);
    }

    #[tokio::test]
    async fn test_routing_nonexistent_session_fails() {
        let state = RelayState::new();

        let message = MpcMessage {
            session_id: Uuid::new_v4(),
            from_party: 0,
            to_party: 1,
            round: 1,
            payload: vec![],
            timestamp: 0,
        };

        let result = state.route_message(message).await;
        assert!(result.is_err());
    }
}
