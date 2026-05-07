use mpc_core::dkls23::KeyShare;
use mpc_core::security::SecureVec;
use sqlx::PgPool;
use uuid::Uuid;

use crate::services::crypto::{EncryptedData, EncryptionService};

use super::types::SERVER_PARTY_INDEX;

/// Persistent storage for the server's encrypted key shares.
pub struct ShardStore {
    db: PgPool,
    encryption: EncryptionService,
}

impl ShardStore {
    pub fn new(db: PgPool, encryption: EncryptionService) -> Self {
        Self { db, encryption }
    }

    /// Store the server's KeyShare after DKG completes.
    /// The secret_share is encrypted at rest with AES-256-GCM.
    pub async fn store_key_share(
        &self,
        user_id: Uuid,
        share: &KeyShare,
    ) -> Result<(), String> {
        let plaintext = self.serialize_share(share)?;
        let encrypted = self.encryption.encrypt(&plaintext)
            .map_err(|e| format!("encryption failed: {}", e))?;

        sqlx::query(
            "INSERT INTO shard_metadata
             (user_id, location, party_index, status, encrypted_shard, nonce, encryption_key_id)
             VALUES ($1, 'server', $2, 'healthy', $3, $4, $5)
             ON CONFLICT (user_id, location) DO UPDATE SET
                 encrypted_shard = EXCLUDED.encrypted_shard,
                 nonce = EXCLUDED.nonce,
                 encryption_key_id = EXCLUDED.encryption_key_id,
                 status = 'healthy',
                 last_verified = NOW()"
        )
        .bind(user_id)
        .bind(SERVER_PARTY_INDEX as i16)
        .bind(&encrypted.ciphertext)
        .bind(encrypted.nonce.as_slice())
        .bind(self.encryption.key_id())
        .execute(&self.db)
        .await
        .map_err(|e| format!("DB store failed: {}", e))?;

        tracing::info!("Stored server KeyShare for user {}", user_id);
        Ok(())
    }

    /// Load the server's KeyShare for signing.
    /// Returns None if no shard exists for this user.
    pub async fn load_key_share(&self, user_id: Uuid) -> Result<Option<KeyShare>, String> {
        let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
            "SELECT encrypted_shard, nonce FROM shard_metadata
             WHERE user_id = $1 AND location = 'server'"
        )
        .bind(user_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("DB load failed: {}", e))?;

        let (ciphertext, nonce_vec) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        if nonce_vec.len() != 12 {
            return Err("invalid nonce length in DB".into());
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec);

        let encrypted = EncryptedData { nonce, ciphertext };
        let plaintext = self.encryption.decrypt(&encrypted)
            .map_err(|e| format!("decryption failed: {}", e))?;

        let share = self.deserialize_share(&plaintext)?;

        // Update last_used
        let _ = sqlx::query(
            "UPDATE shard_metadata SET last_used = NOW() WHERE user_id = $1 AND location = 'server'"
        )
        .bind(user_id)
        .execute(&self.db)
        .await;

        Ok(Some(share))
    }

    /// Store the server's KeyShare for a specific wallet after DKG completes.
    /// The secret_share is encrypted at rest with AES-256-GCM.
    pub async fn store_key_share_for_wallet(
        &self,
        user_id: Uuid,
        wallet_id: Uuid,
        share: &KeyShare,
    ) -> Result<(), String> {
        let plaintext = self.serialize_share(share)?;
        let encrypted = self.encryption.encrypt(&plaintext)
            .map_err(|e| format!("encryption failed: {}", e))?;

        sqlx::query(
            "INSERT INTO shard_metadata
             (user_id, wallet_id, location, party_index, status, encrypted_shard, nonce, encryption_key_id)
             VALUES ($1, $2, 'server', $3, 'healthy', $4, $5, $6)
             ON CONFLICT (user_id, wallet_id, location) WHERE wallet_id IS NOT NULL DO UPDATE SET
                 encrypted_shard = EXCLUDED.encrypted_shard,
                 nonce = EXCLUDED.nonce,
                 encryption_key_id = EXCLUDED.encryption_key_id,
                 status = 'healthy',
                 last_verified = NOW()"
        )
        .bind(user_id)
        .bind(wallet_id)
        .bind(SERVER_PARTY_INDEX as i16)
        .bind(&encrypted.ciphertext)
        .bind(encrypted.nonce.as_slice())
        .bind(self.encryption.key_id())
        .execute(&self.db)
        .await
        .map_err(|e| format!("DB store failed: {}", e))?;

        tracing::info!("Stored server KeyShare for user {} wallet {}", user_id, wallet_id);
        Ok(())
    }

    /// Load the server's KeyShare for a specific wallet.
    /// Returns None if no shard exists for this user+wallet combination.
    pub async fn load_key_share_for_wallet(
        &self,
        user_id: Uuid,
        wallet_id: Uuid,
    ) -> Result<Option<KeyShare>, String> {
        let row: Option<(Vec<u8>, Vec<u8>)> = sqlx::query_as(
            "SELECT encrypted_shard, nonce FROM shard_metadata
             WHERE user_id = $1 AND wallet_id = $2 AND location = 'server'"
        )
        .bind(user_id)
        .bind(wallet_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("DB load failed: {}", e))?;

        let (ciphertext, nonce_vec) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        if nonce_vec.len() != 12 {
            return Err("invalid nonce length in DB".into());
        }
        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&nonce_vec);

        let encrypted = EncryptedData { nonce, ciphertext };
        let plaintext = self.encryption.decrypt(&encrypted)
            .map_err(|e| format!("decryption failed: {}", e))?;

        let share = self.deserialize_share(&plaintext)?;

        // Update last_used
        let _ = sqlx::query(
            "UPDATE shard_metadata SET last_used = NOW()
             WHERE user_id = $1 AND wallet_id = $2 AND location = 'server'"
        )
        .bind(user_id)
        .bind(wallet_id)
        .execute(&self.db)
        .await;

        Ok(Some(share))
    }

    /// List all wallet IDs associated with a user's server shards.
    pub async fn list_wallets(&self, user_id: Uuid) -> Result<Vec<Uuid>, String> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            "SELECT DISTINCT w.id FROM wallets w
             INNER JOIN shard_metadata sm ON sm.wallet_id = w.id
             WHERE w.user_id = $1 AND sm.location = 'server' AND w.status = 'active'
             ORDER BY w.id"
        )
        .bind(user_id)
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("DB list wallets failed: {}", e))?;

        Ok(rows.into_iter().map(|(id,)| id).collect())
    }

    /// Check if a server shard already exists for this user.
    pub async fn has_key_share(&self, user_id: Uuid) -> bool {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM shard_metadata WHERE user_id = $1 AND location = 'server')"
        )
        .bind(user_id)
        .fetch_one(&self.db)
        .await
        .unwrap_or(false)
    }

    fn serialize_share(&self, share: &KeyShare) -> Result<Vec<u8>, String> {
        // Format: [party:2][threshold:2][total:2][pubkey_len:4][pubkey][secret:32]
        let mut buf = Vec::with_capacity(10 + share.public_key.len() + 32);
        buf.extend_from_slice(&share.party.to_le_bytes());
        buf.extend_from_slice(&share.threshold.to_le_bytes());
        buf.extend_from_slice(&share.total_parties.to_le_bytes());
        buf.extend_from_slice(&(share.public_key.len() as u32).to_le_bytes());
        buf.extend_from_slice(&share.public_key);
        buf.extend_from_slice(share.secret_share.as_bytes());
        Ok(buf)
    }

    fn deserialize_share(&self, data: &[u8]) -> Result<KeyShare, String> {
        if data.len() < 10 {
            return Err("share data too short".into());
        }
        let party = u16::from_le_bytes([data[0], data[1]]);
        let threshold = u16::from_le_bytes([data[2], data[3]]);
        let total_parties = u16::from_le_bytes([data[4], data[5]]);
        let pubkey_len = u32::from_le_bytes([data[6], data[7], data[8], data[9]]) as usize;

        if data.len() < 10 + pubkey_len + 32 {
            return Err("share data truncated".into());
        }
        let public_key = data[10..10 + pubkey_len].to_vec();
        let secret_bytes = data[10 + pubkey_len..10 + pubkey_len + 32].to_vec();

        Ok(KeyShare {
            party,
            threshold,
            total_parties,
            secret_share: SecureVec::from(secret_bytes),
            public_key,
            paillier_pk: None,
        })
    }
}
