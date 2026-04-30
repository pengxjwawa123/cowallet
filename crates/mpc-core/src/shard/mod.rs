pub mod backup;
pub mod device;
pub mod encrypt;
pub mod server;

use serde::{Deserialize, Serialize};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Which party holds this shard.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardLocation {
    Device,
    Server,
    Backup,
}

/// An encrypted shard that is safe to store at rest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedShard {
    pub location: ShardLocation,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub party_index: u16,
}

/// A decrypted shard — exists only transiently in Rust memory during signing.
#[derive(Zeroize, ZeroizeOnDrop, Clone)]
pub struct DecryptedShard {
    pub party_index: u16,
    pub secret_share: Vec<u8>,
}

impl DecryptedShard {
    /// Create a new decrypted shard from bytes.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        Self {
            party_index: 0,
            secret_share: bytes,
        }
    }

    /// Get the raw bytes of the shard.
    pub fn as_bytes(&self) -> &[u8] {
        &self.secret_share
    }
}

/// Health status of a shard.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardHealth {
    pub location: ShardLocation,
    pub status: ShardStatus,
    pub last_used: Option<chrono::DateTime<chrono::Utc>>,
    pub last_verified: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardStatus {
    Healthy,
    NeedsVerification,
    Compromised,
    Missing,
}
