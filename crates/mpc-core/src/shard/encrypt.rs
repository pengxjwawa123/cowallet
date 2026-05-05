use super::{DecryptedShard, EncryptedShard, ShardLocation};
use crate::errors::{MpcError, Result};
use aes_gcm::aead::Aead;
use aes_gcm::{
    Aes256Gcm, Nonce,
    aead::{KeyInit, Payload},
};
use rand::RngCore;

const NONCE_SIZE: usize = 12;

fn build_aad(party_index: u16, location: ShardLocation) -> Vec<u8> {
    let loc_byte = match location {
        ShardLocation::Device => 0u8,
        ShardLocation::Server => 1u8,
        ShardLocation::Backup => 2u8,
    };
    let mut aad = Vec::with_capacity(3);
    aad.extend_from_slice(&party_index.to_le_bytes());
    aad.push(loc_byte);
    aad
}

/// Encrypt a shard with AES-256-GCM.
///
/// The `wrapping_key` is derived from:
/// - Device shard: Secure Enclave ECDH (hardware-bound)
/// - Server shard: HSM internal key
/// - Backup shard: Argon2id(user_password, salt)
///
/// AAD binds (party_index, location) to the ciphertext, preventing
/// metadata-swap attacks that could leak key information.
pub fn encrypt_shard(
    shard: &DecryptedShard,
    wrapping_key: &[u8; 32],
    location: ShardLocation,
) -> Result<EncryptedShard> {
    let cipher = Aes256Gcm::new_from_slice(wrapping_key)
        .map_err(|e| MpcError::ShardEncryption(e.to_string()))?;

    let mut nonce_bytes = [0u8; NONCE_SIZE];
    rand::thread_rng().fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let aad = build_aad(shard.party_index, location);
    let payload = Payload {
        msg: shard.secret_share.as_ref(),
        aad: &aad,
    };

    let ciphertext = cipher
        .encrypt(nonce, payload)
        .map_err(|e| MpcError::ShardEncryption(e.to_string()))?;

    Ok(EncryptedShard {
        location,
        nonce: nonce_bytes.to_vec(),
        ciphertext,
        party_index: shard.party_index,
    })
}

/// Decrypt a shard with AES-256-GCM.
///
/// The wrapping_key must match the one used for encryption.
/// AAD is re-derived from the stored party_index and location —
/// tampered metadata causes decryption failure.
pub fn decrypt_shard(
    encrypted: &EncryptedShard,
    wrapping_key: &[u8; 32],
) -> Result<DecryptedShard> {
    let cipher = Aes256Gcm::new_from_slice(wrapping_key)
        .map_err(|e| MpcError::ShardDecryption(e.to_string()))?;

    let nonce = Nonce::from_slice(&encrypted.nonce);

    let aad = build_aad(encrypted.party_index, encrypted.location);
    let payload = Payload {
        msg: encrypted.ciphertext.as_ref(),
        aad: &aad,
    };

    let plaintext = cipher
        .decrypt(nonce, payload)
        .map_err(|e| MpcError::ShardDecryption(e.to_string()))?;

    Ok(DecryptedShard {
        party_index: encrypted.party_index,
        secret_share: plaintext.into(),
    })
}

/// Derive a wrapping key from a user password using Argon2id.
/// Used for backup shard encryption.
pub fn derive_key_from_password(password: &[u8], salt: &[u8; 16]) -> Result<[u8; 32]> {
    use argon2::Argon2;

    let mut key = [0u8; 32];
    Argon2::default()
        .hash_password_into(password, salt, &mut key)
        .map_err(|e| MpcError::ShardEncryption(e.to_string()))?;

    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let shard = DecryptedShard {
            party_index: 0,
            secret_share: vec![1, 2, 3, 4, 5, 6, 7, 8].into(),
        };

        let encrypted = encrypt_shard(&shard, &key, ShardLocation::Device).unwrap();
        let decrypted = decrypt_shard(&encrypted, &key).unwrap();

        assert_eq!(decrypted.secret_share, shard.secret_share);
        assert_eq!(decrypted.party_index, shard.party_index);
    }

    #[test]
    fn test_wrong_key_fails() {
        let key = [42u8; 32];
        let wrong_key = [99u8; 32];
        let shard = DecryptedShard {
            party_index: 1,
            secret_share: vec![10, 20, 30].into(),
        };

        let encrypted = encrypt_shard(&shard, &key, ShardLocation::Server).unwrap();
        let result = decrypt_shard(&encrypted, &wrong_key);

        assert!(result.is_err());
    }

    #[test]
    fn test_password_key_derivation() {
        let password = b"correct horse battery staple";
        let salt = [0u8; 16];
        let key = derive_key_from_password(password, &salt).unwrap();
        assert_eq!(key.len(), 32);

        // Deterministic: same input → same key
        let key2 = derive_key_from_password(password, &salt).unwrap();
        assert_eq!(key, key2);
    }
}
