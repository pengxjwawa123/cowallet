use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{Aead, KeyInit},
};
use rand::RngCore;
use thiserror::Error;

/// Encrypted local storage for non-shard sensitive data.
///
/// Used for: chat history, cached balances, user preferences,
/// agent rules — anything that should be encrypted at rest but
/// doesn't need Secure Enclave-level protection.
pub struct EncryptedStore {
    cipher: Aes256Gcm,
}

impl Drop for EncryptedStore {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        let ptr = &mut self.cipher as *mut Aes256Gcm as *mut u8;
        let len = std::mem::size_of::<Aes256Gcm>();
        unsafe {
            std::slice::from_raw_parts_mut(ptr, len).zeroize();
        }
    }
}

impl EncryptedStore {
    pub fn new(key: &[u8; 32]) -> Self {
        let cipher = Aes256Gcm::new_from_slice(key).expect("valid key length");
        Self { cipher }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<Vec<u8>, StoreError> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| StoreError::Encryption(e.to_string()))?;

        // Prepend nonce to ciphertext for storage
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, data: &[u8]) -> Result<Vec<u8>, StoreError> {
        if data.len() < 12 {
            return Err(StoreError::Decryption("data too short".into()));
        }

        let nonce = Nonce::from_slice(&data[..12]);
        let ciphertext = &data[12..];

        self.cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| StoreError::Decryption(e.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("encryption failed: {0}")]
    Encryption(String),

    #[error("decryption failed: {0}")]
    Decryption(String),

    #[error("IO error: {0}")]
    Io(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let key = [42u8; 32];
        let store = EncryptedStore::new(&key);
        let plaintext = b"hello cowallet";

        let encrypted = store.encrypt(plaintext).unwrap();
        let decrypted = store.decrypt(&encrypted).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key1 = [1u8; 32];
        let key2 = [2u8; 32];
        let store1 = EncryptedStore::new(&key1);
        let store2 = EncryptedStore::new(&key2);

        let encrypted = store1.encrypt(b"secret").unwrap();
        assert!(store2.decrypt(&encrypted).is_err());
    }
}
