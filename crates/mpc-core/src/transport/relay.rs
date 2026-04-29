use crate::dkls23::ProtocolMessage;
use crate::errors::{MpcError, Result};

/// NATS-based message relay for MPC protocol communication.
///
/// Messages are published to per-session NATS subjects:
///   cowallet.mpc.{session_id}.{to_party}
///
/// Each party subscribes to their own subject and receives messages
/// from other parties in the same session.
pub struct NatsRelay {
    // TODO: async_nats::Client
    _client: (),
}

impl NatsRelay {
    /// Connect to the NATS server.
    pub async fn connect(_url: &str) -> Result<Self> {
        // TODO: async_nats::connect(url)
        Err(MpcError::Transport("not yet implemented".into()))
    }

    /// Publish a protocol message to the target party's subject.
    pub async fn publish(&self, msg: &ProtocolMessage) -> Result<()> {
        let _subject = format!("cowallet.mpc.{}.{}", msg.session_id, msg.to);
        // TODO: self.client.publish(subject, payload).await
        Err(MpcError::Transport("not yet implemented".into()))
    }

    /// Subscribe to messages for this party in a session.
    ///
    /// Returns a receiver channel that yields incoming protocol messages.
    pub async fn subscribe(
        &self,
        _session_id: &str,
        _party_index: u16,
    ) -> Result<tokio::sync::mpsc::Receiver<ProtocolMessage>> {
        // TODO: self.client.subscribe(subject).await, deserialize, forward to channel
        Err(MpcError::Transport("not yet implemented".into()))
    }
}

/// In-memory transport for testing. Routes messages directly between
/// parties without network IO.
#[cfg(test)]
pub struct InMemoryTransport {
    // TODO: Arc<Mutex<HashMap<(session, party), VecDeque<ProtocolMessage>>>>
}
