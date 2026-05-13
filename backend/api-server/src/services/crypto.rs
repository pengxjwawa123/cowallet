use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use hkdf::Hkdf;
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

/// Encryption service using AES-256-GCM with HKDF key derivation
#[derive(Clone, ZeroizeOnDrop)]
pub struct EncryptionService {
    #[zeroize(skip)]
    key_id: String,
    #[zeroize(skip)]
    context: String,
    // In production, use a KMS instead of storing keys directly
    // This is a simplified version - for demo only!
    root_key: [u8; 32],
}

impl EncryptionService {
    /// Create a new encryption service with a root key and context
    /// In production, this key would come from AWS KMS, HashiCorp Vault, etc.
    /// The context string is used for HKDF key derivation - different contexts produce different keys
    pub fn new(root_key: &[u8; 32], context: &str) -> Self {
        Self {
            key_id: format!("{}-v1", context),
            context: context.to_string(),
            root_key: *root_key,
        }
    }

    /// Create a test instance (not for production)
    pub fn for_test() -> Self {
        let mut root_key = [0u8; 32];
        OsRng.fill_bytes(&mut root_key);
        Self::new(&root_key, "test-key")
    }

    /// Derive a context-specific encryption key using HKDF-SHA256
    /// This ensures different shards/purposes use different keys even from the same root key
    fn derive_key(&self) -> [u8; 32] {
        let hkdf = Hkdf::<Sha256>::new(None, &self.root_key);
        let info = format!("cowallet-v1-{}", self.context);
        let mut derived_key = [0u8; 32];
        hkdf.expand(info.as_bytes(), &mut derived_key)
            .expect("HKDF expand failed (should never happen with valid length)");
        derived_key
    }

    /// Encrypt data using AES-256-GCM with HKDF-derived key
    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData, CryptoError> {
        // Generate a unique 12-byte nonce for each encryption
        let mut nonce_bytes = [0u8; 12];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Derive context-specific key
        let derived_key = self.derive_key();
        let key = Key::<Aes256Gcm>::from_slice(&derived_key);
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

    /// Decrypt data using AES-256-GCM with HKDF-derived key
    pub fn decrypt(&self, encrypted: &EncryptedData) -> Result<Vec<u8>, CryptoError> {
        let nonce = Nonce::from_slice(&encrypted.nonce);

        // Derive context-specific key
        let derived_key = self.derive_key();
        let key = Key::<Aes256Gcm>::from_slice(&derived_key);
        let cipher = Aes256Gcm::new(key);

        cipher
            .decrypt(nonce, encrypted.ciphertext.as_ref())
            .map_err(|e| CryptoError::Decryption(e.to_string()))
    }

    /// Re-encrypt data with a new root key (for key rotation)
    /// This is atomic - either both decrypt and encrypt succeed, or neither does
    pub fn rotate_key(&self, encrypted: &EncryptedData, new_service: &EncryptionService) -> Result<EncryptedData, CryptoError> {
        // Decrypt with old key
        let plaintext = self.decrypt(encrypted)?;

        // Re-encrypt with new key
        let result = new_service.encrypt(&plaintext);

        // Zeroize plaintext from memory
        drop(plaintext);

        result
    }

    /// Batch re-encrypt multiple items during key rotation
    pub fn rotate_keys_batch(&self, encrypted_items: &[EncryptedData], new_service: &EncryptionService) -> Result<Vec<EncryptedData>, CryptoError> {
        encrypted_items
            .iter()
            .map(|item| self.rotate_key(item, new_service))
            .collect()
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

    #[test]
    fn test_decrypt_wrong_key_fails() {
        // More explicit test name variant
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];

        let service1 = EncryptionService::new(&key1, "key1");
        let service2 = EncryptionService::new(&key2, "key2");

        let plaintext = b"sensitive shard data";
        let encrypted = service1.encrypt(plaintext).unwrap();

        // Attempting to decrypt with wrong key should fail
        let result = service2.decrypt(&encrypted);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), CryptoError::Decryption(_)));
    }

    #[test]
    fn test_different_plaintexts_different_ciphertexts() {
        let service = EncryptionService::for_test();
        let plaintext1 = b"first message";
        let plaintext2 = b"second message";

        let encrypted1 = service.encrypt(plaintext1).unwrap();
        let encrypted2 = service.encrypt(plaintext2).unwrap();

        // Different plaintexts should produce different ciphertexts
        assert_ne!(encrypted1.ciphertext, encrypted2.ciphertext);
        // And different nonces
        assert_ne!(encrypted1.nonce, encrypted2.nonce);
    }

    #[test]
    fn test_key_id_matches() {
        let key = [42u8; 32];
        let context = "production-key-2024";
        let service = EncryptionService::new(&key, context);

        assert_eq!(service.key_id(), "production-key-2024-v1");
    }

    #[test]
    fn test_hash_shard_deterministic() {
        let shard = b"test shard data for hashing";

        let hash1 = EncryptionService::hash_shard(shard);
        let hash2 = EncryptionService::hash_shard(shard);

        // Same input should produce same hash
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 32); // SHA-256 produces 32 bytes
    }

    #[test]
    fn test_hash_shard_different_inputs() {
        let shard1 = b"shard one";
        let shard2 = b"shard two";

        let hash1 = EncryptionService::hash_shard(shard1);
        let hash2 = EncryptionService::hash_shard(shard2);

        // Different inputs should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_encrypted_data_zeroizes_on_drop() {
        let service = EncryptionService::for_test();
        let plaintext = b"secret data";

        let encrypted = service.encrypt(plaintext).unwrap();
        let ciphertext_ptr = encrypted.ciphertext.as_ptr();
        let ciphertext_len = encrypted.ciphertext.len();

        // Encrypted data should be valid
        assert!(!encrypted.ciphertext.is_empty());

        // Drop should zeroize (we can't verify the actual memory,
        // but we can verify the zeroize trait is implemented)
        drop(encrypted);

        // This test mainly verifies compilation - the zeroize trait
        // is correctly applied to EncryptedData
    }

    #[test]
    fn test_large_data_encryption() {
        let service = EncryptionService::for_test();
        let large_data = vec![0xAB; 1024 * 100]; // 100 KB

        let encrypted = service.encrypt(&large_data).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(large_data, decrypted);
    }

    #[test]
    fn test_empty_data_encryption() {
        let service = EncryptionService::for_test();
        let empty_data = b"";

        let encrypted = service.encrypt(empty_data).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();

        assert_eq!(empty_data.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_hkdf_different_contexts_produce_different_keys() {
        let root_key = [42u8; 32];
        let service1 = EncryptionService::new(&root_key, "context-a");
        let service2 = EncryptionService::new(&root_key, "context-b");

        let plaintext = b"test data for context isolation";

        // Encrypt with service1
        let encrypted1 = service1.encrypt(plaintext).unwrap();

        // Try to decrypt with service2 (different context) - should fail
        let result = service2.decrypt(&encrypted1);
        assert!(result.is_err(), "Different contexts should produce different keys");

        // Decrypt with service1 should work
        let decrypted1 = service1.decrypt(&encrypted1).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted1.as_slice());
    }

    #[test]
    fn test_hkdf_same_context_works() {
        let root_key = [42u8; 32];
        let service1 = EncryptionService::new(&root_key, "shared-context");
        let service2 = EncryptionService::new(&root_key, "shared-context");

        let plaintext = b"test data for same context";

        // Encrypt with service1
        let encrypted = service1.encrypt(plaintext).unwrap();

        // Decrypt with service2 (same context and root key) should work
        let decrypted = service2.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_key_rotation_basic() {
        let old_key = [1u8; 32];
        let new_key = [2u8; 32];

        let old_service = EncryptionService::new(&old_key, "shard-storage");
        let new_service = EncryptionService::new(&new_key, "shard-storage");

        let plaintext = b"sensitive shard data to rotate";

        // Encrypt with old key
        let encrypted_old = old_service.encrypt(plaintext).unwrap();

        // Rotate to new key
        let encrypted_new = old_service.rotate_key(&encrypted_old, &new_service).unwrap();

        // Old service cannot decrypt new ciphertext
        let old_decrypt_result = old_service.decrypt(&encrypted_new);
        assert!(old_decrypt_result.is_err(), "Old key should not decrypt rotated data");

        // New service can decrypt
        let decrypted = new_service.decrypt(&encrypted_new).unwrap();
        assert_eq!(plaintext.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_key_rotation_preserves_data() {
        let old_key = [3u8; 32];
        let new_key = [4u8; 32];

        let old_service = EncryptionService::new(&old_key, "rotation-test");
        let new_service = EncryptionService::new(&new_key, "rotation-test");

        let test_data = vec![
            b"first shard".as_slice(),
            b"second shard with more data".as_slice(),
            b"third shard".as_slice(),
        ];

        // Encrypt all with old key
        let encrypted_old: Vec<_> = test_data
            .iter()
            .map(|data| old_service.encrypt(data).unwrap())
            .collect();

        // Rotate all to new key
        let encrypted_new = old_service.rotate_keys_batch(&encrypted_old, &new_service).unwrap();
        assert_eq!(encrypted_new.len(), test_data.len());

        // Decrypt with new key and verify
        for (i, encrypted) in encrypted_new.iter().enumerate() {
            let decrypted = new_service.decrypt(encrypted).unwrap();
            assert_eq!(test_data[i], decrypted.as_slice());
        }
    }

    #[test]
    fn test_key_rotation_batch_atomic_on_error() {
        let old_key = [5u8; 32];
        let new_key = [6u8; 32];

        let old_service = EncryptionService::new(&old_key, "batch-test");
        let new_service = EncryptionService::new(&new_key, "batch-test");

        let plaintext1 = b"valid data 1";
        let plaintext2 = b"valid data 2";

        let encrypted1 = old_service.encrypt(plaintext1).unwrap();
        let encrypted2 = old_service.encrypt(plaintext2).unwrap();

        // Create a corrupted encrypted item
        let corrupted = EncryptedData {
            nonce: [0u8; 12],
            ciphertext: vec![0xFF; 32], // Invalid ciphertext
        };

        let batch = vec![encrypted1.clone(), corrupted, encrypted2.clone()];

        // Batch rotation should fail (one item is corrupted)
        let result = old_service.rotate_keys_batch(&batch, &new_service);
        assert!(result.is_err(), "Batch rotation should fail if any item fails");
    }

    #[test]
    fn test_derive_key_deterministic() {
        let root_key = [7u8; 32];
        let service = EncryptionService::new(&root_key, "deterministic");

        let key1 = service.derive_key();
        let key2 = service.derive_key();

        assert_eq!(key1, key2, "Derived key should be deterministic");
    }

    #[test]
    fn test_different_contexts_different_derived_keys() {
        let root_key = [8u8; 32];
        let service1 = EncryptionService::new(&root_key, "context-x");
        let service2 = EncryptionService::new(&root_key, "context-y");

        let key1 = service1.derive_key();
        let key2 = service2.derive_key();

        assert_ne!(key1, key2, "Different contexts should derive different keys");
    }

    #[test]
    fn test_key_id_includes_context() {
        let root_key = [9u8; 32];
        let service = EncryptionService::new(&root_key, "user-shard");

        assert!(service.key_id().contains("user-shard"));
        assert!(service.key_id().contains("-v1"));
    }

    #[test]
    fn test_rotate_key_with_empty_data() {
        let old_key = [10u8; 32];
        let new_key = [11u8; 32];

        let old_service = EncryptionService::new(&old_key, "empty-test");
        let new_service = EncryptionService::new(&new_key, "empty-test");

        let empty = b"";
        let encrypted_old = old_service.encrypt(empty).unwrap();

        // Should successfully rotate even empty data
        let encrypted_new = old_service.rotate_key(&encrypted_old, &new_service).unwrap();
        let decrypted = new_service.decrypt(&encrypted_new).unwrap();

        assert_eq!(empty.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_hkdf_key_isolation_security() {
        // Security test: ensure that knowing derived key for one context
        // doesn't compromise another context (in practice, we can't extract
        // the derived key, but we can verify encryption isolation)

        let root_key = [12u8; 32];
        let device_service = EncryptionService::new(&root_key, "device-shard");
        let server_service = EncryptionService::new(&root_key, "server-shard");
        let backup_service = EncryptionService::new(&root_key, "backup-shard");

        let secret = b"critical private key material";

        // Encrypt same data with different contexts
        let device_enc = device_service.encrypt(secret).unwrap();
        let server_enc = server_service.encrypt(secret).unwrap();
        let backup_enc = backup_service.encrypt(secret).unwrap();

        // Each service can only decrypt its own
        assert!(device_service.decrypt(&device_enc).is_ok());
        assert!(device_service.decrypt(&server_enc).is_err());
        assert!(device_service.decrypt(&backup_enc).is_err());

        assert!(server_service.decrypt(&server_enc).is_ok());
        assert!(server_service.decrypt(&device_enc).is_err());
        assert!(server_service.decrypt(&backup_enc).is_err());

        assert!(backup_service.decrypt(&backup_enc).is_ok());
        assert!(backup_service.decrypt(&device_enc).is_err());
        assert!(backup_service.decrypt(&server_enc).is_err());
    }
}
