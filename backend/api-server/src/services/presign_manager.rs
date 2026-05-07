use std::sync::Arc;
use std::time::Duration;

use k256::elliptic_curve::Field;
use k256::elliptic_curve::sec1::ToEncodedPoint;
use k256::{AffinePoint, ProjectivePoint, Scalar};
use rand::rngs::OsRng;
use sqlx::PgPool;
use tokio::sync::Notify;
use uuid::Uuid;

use crate::services::crypto::{EncryptedData, EncryptionService};

/// Serialized presignature data: server's ephemeral secret k_1 and commitment R_1.
/// Layout: [k_1: 32 bytes (scalar)] [R_1: 33 bytes (compressed SEC1 point)]
const PRESIG_DATA_LEN: usize = 32 + 33;

/// Manages presignature lifecycle: generation, storage, consumption.
/// Pre-computes signing material so that online signing only needs 1 round.
#[derive(Clone)]
pub struct PresignManager {
    db: PgPool,
    encryption: EncryptionService,
    shutdown: Arc<Notify>,
}

/// Decrypted presignature data returned when reserving.
#[derive(Debug)]
pub struct PresignatureData {
    pub id: Uuid,
    /// Server's ephemeral secret scalar k_1.
    pub k: Vec<u8>,
    /// Server's commitment point R_1 = k_1 * G (compressed SEC1, 33 bytes).
    pub big_r: Vec<u8>,
}

impl PresignManager {
    pub fn new(db: PgPool, encryption: EncryptionService) -> Self {
        Self {
            db,
            encryption,
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Generate `count` presignatures for a wallet and store them encrypted in the DB.
    ///
    /// Each presignature consists of:
    /// - An ephemeral secret scalar k_1 (random, from OsRng)
    /// - A commitment R_1 = k_1 * G (the corresponding curve point)
    ///
    /// Both are stored encrypted with AES-256-GCM via the EncryptionService.
    pub async fn generate_presignatures(
        &self,
        user_id: Uuid,
        wallet_id: Uuid,
        count: u32,
    ) -> Result<u32, String> {
        let count = count.min(50); // Cap at 50 per call to avoid abuse

        let mut generated = 0u32;

        for _ in 0..count {
            // Generate ephemeral k_1
            let k = Scalar::random(&mut OsRng);
            let big_r_projective = ProjectivePoint::GENERATOR * k;
            let big_r_affine: AffinePoint = big_r_projective.into();
            let big_r_encoded = big_r_affine.to_encoded_point(true); // compressed

            // Serialize: [k_1 scalar bytes (32)] [R_1 compressed point (33)]
            let k_bytes = k.to_bytes();
            let r_bytes = big_r_encoded.as_bytes();

            let mut plaintext = Vec::with_capacity(PRESIG_DATA_LEN);
            plaintext.extend_from_slice(&k_bytes);
            plaintext.extend_from_slice(r_bytes);

            // Encrypt with AES-256-GCM
            let encrypted = self.encryption.encrypt(&plaintext)
                .map_err(|e| format!("encryption failed: {}", e))?;

            // Combine nonce + ciphertext for DB storage
            let mut presig_data = Vec::with_capacity(12 + encrypted.ciphertext.len());
            presig_data.extend_from_slice(&encrypted.nonce);
            presig_data.extend_from_slice(&encrypted.ciphertext);

            // Store in presignatures table
            sqlx::query(
                "INSERT INTO presignatures (wallet_id, user_id, presig_data, status, expires_at)
                 VALUES ($1, $2, $3, 'available', NOW() + INTERVAL '24 hours')"
            )
            .bind(wallet_id)
            .bind(user_id)
            .bind(&presig_data)
            .execute(&self.db)
            .await
            .map_err(|e| format!("DB insert failed: {}", e))?;

            generated += 1;
        }

        tracing::info!(
            "Generated {} presignatures for wallet {} (user {})",
            generated, wallet_id, user_id
        );

        Ok(generated)
    }

    /// Reserve one available presignature for a signing session.
    ///
    /// Uses SELECT ... FOR UPDATE SKIP LOCKED to avoid contention.
    /// Returns the decrypted presignature data (k_1 scalar + R_1 point).
    pub async fn reserve_presignature(
        &self,
        wallet_id: Uuid,
        session_id: Uuid,
    ) -> Result<Option<PresignatureData>, String> {
        // Atomic reserve: find an available presignature and mark it reserved
        let row: Option<(Uuid, Vec<u8>)> = sqlx::query_as(
            "UPDATE presignatures
             SET status = 'reserved', reserved_by = $2
             WHERE id = (
                 SELECT id FROM presignatures
                 WHERE wallet_id = $1 AND status = 'available' AND expires_at > NOW()
                 ORDER BY created_at ASC
                 LIMIT 1
                 FOR UPDATE SKIP LOCKED
             )
             RETURNING id, presig_data"
        )
        .bind(wallet_id)
        .bind(session_id)
        .fetch_optional(&self.db)
        .await
        .map_err(|e| format!("DB reserve failed: {}", e))?;

        let (presig_id, presig_data) = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        // Decrypt the presignature data
        let data = self.decrypt_presig_data(&presig_data)?;

        Ok(Some(PresignatureData {
            id: presig_id,
            k: data.0,
            big_r: data.1,
        }))
    }

    /// Mark a presignature as consumed after a successful signing operation.
    pub async fn consume_presignature(&self, presig_id: Uuid) -> Result<(), String> {
        sqlx::query(
            "UPDATE presignatures SET status = 'consumed', consumed_at = NOW()
             WHERE id = $1 AND status = 'reserved'"
        )
        .bind(presig_id)
        .execute(&self.db)
        .await
        .map_err(|e| format!("DB consume failed: {}", e))?;

        tracing::debug!("Consumed presignature {}", presig_id);
        Ok(())
    }

    /// Mark expired presignatures (past their expires_at) as 'expired'.
    pub async fn cleanup_expired(&self) -> Result<u64, String> {
        let result = sqlx::query(
            "UPDATE presignatures SET status = 'expired'
             WHERE status = 'available' AND expires_at <= NOW()"
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("DB cleanup failed: {}", e))?;

        let count = result.rows_affected();
        if count > 0 {
            tracing::info!("Expired {} presignatures", count);
        }
        Ok(count)
    }

    /// Also release presignatures that have been reserved too long (>10 min)
    /// without being consumed — likely from failed sessions.
    pub async fn cleanup_stale_reservations(&self) -> Result<u64, String> {
        let result = sqlx::query(
            "UPDATE presignatures SET status = 'expired'
             WHERE status = 'reserved'
             AND created_at < NOW() - INTERVAL '10 minutes'
             AND consumed_at IS NULL"
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("DB stale cleanup failed: {}", e))?;

        let count = result.rows_affected();
        if count > 0 {
            tracing::info!("Expired {} stale reserved presignatures", count);
        }
        Ok(count)
    }

    /// Get the count of available presignatures for a wallet.
    pub async fn get_available_count(&self, wallet_id: Uuid) -> Result<i64, String> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM presignatures
             WHERE wallet_id = $1 AND status = 'available' AND expires_at > NOW()"
        )
        .bind(wallet_id)
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("DB count failed: {}", e))?;

        Ok(count)
    }

    /// Ensure a minimum number of presignatures are available for a wallet.
    /// If the available count is below `min_count`, generate enough to reach it.
    pub async fn ensure_minimum(
        &self,
        wallet_id: Uuid,
        user_id: Uuid,
        min_count: u32,
    ) -> Result<(), String> {
        let available = self.get_available_count(wallet_id).await?;

        if (available as u32) < min_count {
            let deficit = min_count - available as u32;
            tracing::debug!(
                "Wallet {} has {} presignatures, need {}, generating {}",
                wallet_id, available, min_count, deficit
            );
            self.generate_presignatures(user_id, wallet_id, deficit).await?;
        }

        Ok(())
    }

    /// Spawn a background task that periodically:
    /// 1. Cleans up expired presignatures
    /// 2. Cleans up stale reservations
    /// 3. Ensures minimum presignature counts for active wallets
    pub fn spawn_background_task(self: &Arc<Self>, min_presignatures: u32) {
        let this = Arc::clone(self);
        let interval_secs = std::env::var("PRESIGN_REFRESH_INTERVAL_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(60u64);

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        // 1. Cleanup expired
                        if let Err(e) = this.cleanup_expired().await {
                            tracing::error!("Presign cleanup_expired failed: {}", e);
                        }

                        // 2. Cleanup stale reservations
                        if let Err(e) = this.cleanup_stale_reservations().await {
                            tracing::error!("Presign cleanup_stale failed: {}", e);
                        }

                        // 3. Top up active wallets
                        if let Err(e) = this.topup_active_wallets(min_presignatures).await {
                            tracing::error!("Presign topup failed: {}", e);
                        }
                    }
                    _ = this.shutdown.notified() => {
                        tracing::info!("PresignManager background task shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Top up presignatures for all active wallets that are below the minimum.
    async fn topup_active_wallets(&self, min_count: u32) -> Result<(), String> {
        // Query all active wallets
        let wallets: Vec<(Uuid, Uuid)> = sqlx::query_as(
            "SELECT id, user_id FROM wallets WHERE status = 'active'"
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("DB fetch wallets failed: {}", e))?;

        for (wallet_id, user_id) in wallets {
            if let Err(e) = self.ensure_minimum(wallet_id, user_id, min_count).await {
                tracing::warn!(
                    "Failed to ensure minimum presignatures for wallet {}: {}",
                    wallet_id, e
                );
            }
        }

        Ok(())
    }

    /// Signal the background task to stop.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }

    /// Decrypt stored presig_data bytes into (k_bytes, R_bytes).
    fn decrypt_presig_data(&self, stored: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
        if stored.len() < 12 {
            return Err("presig_data too short (missing nonce)".into());
        }

        let mut nonce = [0u8; 12];
        nonce.copy_from_slice(&stored[..12]);
        let ciphertext = stored[12..].to_vec();

        let encrypted = EncryptedData { nonce, ciphertext };
        let plaintext = self.encryption.decrypt(&encrypted)
            .map_err(|e| format!("presig decryption failed: {}", e))?;

        if plaintext.len() != PRESIG_DATA_LEN {
            return Err(format!(
                "presig plaintext wrong size: expected {}, got {}",
                PRESIG_DATA_LEN,
                plaintext.len()
            ));
        }

        let k_bytes = plaintext[..32].to_vec();
        let r_bytes = plaintext[32..].to_vec();

        Ok((k_bytes, r_bytes))
    }
}
