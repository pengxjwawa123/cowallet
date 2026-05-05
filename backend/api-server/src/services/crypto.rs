use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use rand::RngCore;
use sha2::{Sha256, Digest};
use zeroize::{Zeroize, ZeroizeOnDrop};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CryptoError {
    #[error("Encryption failed: {0}")]
    Encryption(String),
    #[error("Decryption failed: {0}")]
    Decryption(String),
    #[error("Invalid key length")]
    InvalidKeyLength,
}

/// Encrypted data bundle with nonce
#[derive(Clone, Zeroize)]
pub struct EncryptedData {
    #[zeroize(skip)]
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Encryption service using AES-256-GCM
#[derive(Clone, ZeroizeOnDrop)]
pub struct EncryptionService {
    #[zeroize(skip)]
    key_id: String,
    // In production, use a KMS instead of storing keys directly
    // This is a simplified version - for demo only!
    root_key: [u8; 32],
}

impl EncryptionService {
    /// Create a new encryption service with a root key
    /// In production, this key would come from AWS KMS, HashiCorp Vault, etc.
    pub fn new(root_key: &[u8; 32], key_id: &str) -> Self {
        Self {
            key_id: key_id.to_string(),
            root_key: *root_key,
        }
    }

    /// Create a test instance (not for production)
    pub fn for_test() -> Self {
        let mut root_key = [0u8; 32];
        OsRng.fill_bytes(&mut root_key);
        Self::new(&root_key, "test-key")
    }

    /// Encrypt data using AES-256-GCM
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError> {
        // Generate a unique 12-byte nonce for each encryption
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Create cipher
        let key = Key::<Aes256Gcm>::from_slice(&self.root_key);
        let cipher = Aes256Gcm::new(key);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| CryptoError::Encryption(e.to_string()))?;

        Ok(EncryptedData {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Decrypt data using AES-256-GCM
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(&encrypted.nonce);
        let key = Key::<Aes256Gcm>::from_slice(&self.root_key);
        let cipher = Aes256Gcm::new(key);

        cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }

    /// Hash a shard for integrity verification (SHA-256)
    pub fn hash_shard(shard: &[u8]) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hasher.update(shard);
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Get the key ID
    pub fn key_id(&self) -> &str {
        &self.key_id
    }
}

impl Drop for EncryptedData {
    fn drop(&mut self) {
        self.ciphertext.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let service = EncryptionService::for_test();
        let plaintext = b"test key shard data";

        let encrypted = service.encrypt(plaintext).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_different_nonces_produce_different_ciphertexts() {
        let service = EncryptionService::for_test();
        let plaintext = b"test data";

        let e1 = service.encrypt(plaintext).unwrap();
        let e2 = service.encrypt(plaintext).unwrap();

        assert_ne!(e1.ciphertext, e2.ciphertext);
        assert_ne!(e1.nonce, e2.nonce);
    }

    #[test]
    fn test_wrong_key_fails_decryption() {
        let service1 = EncryptionService::for_test();
        let service2 = EncryptionService::for_test();
        let plaintext = b"test data";

        let encrypted = service1.encrypt(plaintext).unwrap();
        let result = service2.decrypt(&encrypted);

        assert!(result.is_err());
    }
}
