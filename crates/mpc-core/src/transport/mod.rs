pub mod noise;
pub mod relay;

use crate::dkls23::ProtocolMessage;
use crate::errors::Result;

/// Trait for sending and receiving MPC protocol messages between parties.
///
/// Implementations:
/// - `NatsRelay`: production transport via NATS server
/// - `InMemoryTransport`: for testing (direct function calls between parties)
#[async_trait::async_trait]
pub trait Transport: Send + Sync {
    /// Send a protocol message to a specific party.
    async fn send(&self, msg: ProtocolMessage) -> Result<()>;

    /// Receive the next protocol message for this party in the given session.
    /// Blocks until a message arrives or timeout.
    async fn recv(&self, session_id: &str, timeout_ms: u64) -> Result<ProtocolMessage>;
}
