use super::{DecryptedShard, EncryptedShard, ShardHealth, ShardLocation, ShardStatus};
use crate::errors::{MpcError, Result};

/// Manages the device shard (Shard 1) stored in Secure Enclave / StrongBox.
///
/// The actual SE/Keystore operations are performed via platform channels
/// (iOS Swift / Android Kotlin). This module handles the Rust side of the
/// encryption/decryption flow using the SE-derived wrapping key.
pub struct DeviceShardManager {
    encrypted_shard: Option<EncryptedShard>,
}

/// Trait for platform-specific Secure Enclave / Keystore operations.
/// Implemented by Flutter platform channels via FFI.
pub trait SecureHardware: Send + Sync {
    /// Derive a wrapping key using SE ECDH. Triggers biometric prompt.
    fn derive_wrapping_key(&self) -> Result<[u8; 32]>;

    /// Check if the hardware security module is available.
    fn is_available(&self) -> bool;
}

impl DeviceShardManager {
    pub fn new() -> Self {
        Self {
            encrypted_shard: None,
        }
    }

    /// Store an encrypted device shard.
    pub fn store(&mut self, shard: EncryptedShard) {
        self.encrypted_shard = Some(shard);
    }

    /// Decrypt the device shard. Requires biometric via SecureHardware.
    pub fn unlock(&self, hw: &dyn SecureHardware) -> Result<DecryptedShard> {
        let encrypted = self
            .encrypted_shard
            .as_ref()
            .ok_or(MpcError::ShardDecryption("no device shard stored".into()))?;

        let wrapping_key = hw.derive_wrapping_key()?;
        let decrypted = super::encrypt::decrypt_shard(encrypted, &wrapping_key)?;
        Ok(decrypted)
    }

    /// Check health status of the device shard.
    pub fn health(&self) -> ShardHealth {
        match &self.encrypted_shard {
            Some(_) => ShardHealth {
                location: ShardLocation::Device,
                status: ShardStatus::Healthy,
                last_used: None,
                last_verified: None,
            },
            None => ShardHealth {
                location: ShardLocation::Device,
                status: ShardStatus::Missing,
                last_used: None,
                last_verified: None,
            },
        }
    }
}

impl Default for DeviceShardManager {
    fn default() -> Self {
        Self::new()
    }
}
