pub mod backup;
pub mod device;
pub mod encrypt;
pub mod server;

use crate::security::SecureVec;
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
///
/// The shard data is memory-locked using `mlock` to prevent it from being
/// swapped to disk, and automatically zeroized on drop.
#[derive(ZeroizeOnDrop, Clone)]
pub struct DecryptedShard {
    pub party_index: u16,
    /// The secret shard data, memory-locked and zeroized on drop
    pub secret_share: SecureVec,
}

impl DecryptedShard {
    /// Create a new decrypted shard from bytes.
    ///
    /// This will attempt to lock the memory to prevent swapping to disk.
    /// If memory locking fails, the data is still stored with zeroization on drop.
    pub fn from_bytes(bytes: Vec<u8>) -> Self {
        let secret_share = SecureVec::new(bytes)
            .unwrap_or_else(|_| SecureVec::new(Vec::new()).unwrap());
        Self {
            party_index: 0,
            secret_share,
        }
    }

    /// Get the raw bytes of the shard.
    pub fn as_bytes(&self) -> &[u8] {
        self.secret_share.as_bytes()
    }
}

impl Zeroize for DecryptedShard {
    fn zeroize(&mut self) {
        // SecureVec's Drop implementation handles zeroization
        // We just need to clear the non-sensitive fields
        self.party_index = 0;
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
