use super::{EncryptedShard, ShardHealth};
use crate::errors::Result;
use serde::{Deserialize, Serialize};

/// Server shard (Shard 2) management.
///
/// The server shard is stored in an HSM (AWS CloudHSM / YubiHSM 2) on the
/// backend. The mobile client never holds this shard — it only interacts
/// with the server via the MPC signing protocol.
///
/// In development, SoftHSM2 is used as a drop-in replacement.

/// Request to store a shard on the server (sent during DKG).
#[derive(Debug, Serialize, Deserialize)]
pub struct StoreShardRequest {
    pub user_id: String,
    pub encrypted_shard: EncryptedShard,
}

/// Server shard status response.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerShardStatus {
    pub exists: bool,
    pub health: ShardHealth,
    pub last_heartbeat: Option<chrono::DateTime<chrono::Utc>>,
}

/// Trait for HSM operations on the server side.
/// Implemented by the backend api-server.
pub trait HsmBackend: Send + Sync {
    /// Store an encrypted shard in the HSM.
    fn store_shard(&self, user_id: &str, shard: &EncryptedShard) -> Result<()>;

    /// Participate in a signing session using the stored shard.
    /// The HSM decrypts the shard internally and performs its part of the
    /// MPC protocol without ever exposing the plaintext shard.
    fn participate_in_signing(&self, user_id: &str, session_id: &str) -> Result<()>;

    /// Check health of the HSM and the stored shard.
    fn health_check(&self, user_id: &str) -> Result<ServerShardStatus>;
}
