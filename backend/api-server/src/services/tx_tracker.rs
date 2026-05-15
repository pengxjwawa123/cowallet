//! Background transaction confirmation tracker.
//!
//! Periodically polls pending/broadcast transactions via `eth_getTransactionReceipt`
//! and updates their status in the database.
//!
//! Polling strategy:
//! - Every 5 seconds for the first 2 minutes after broadcast
//! - Every 30 seconds after that
//! - Transactions older than 1 hour are marked as "dropped"

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use sqlx::PgPool;
use tokio::sync::Notify;

/// Tracks pending transactions and updates their on-chain confirmation status.
#[derive(Clone)]
pub struct TxTracker {
    db: PgPool,
    http: reqwest::Client,
    rpc_urls: HashMap<u64, String>,
    default_rpc: String,
    shutdown: Arc<Notify>,
}

impl TxTracker {
    pub fn new(
        db: PgPool,
        http: reqwest::Client,
        rpc_urls: HashMap<u64, String>,
        default_rpc: String,
    ) -> Self {
        Self {
            db,
            http,
            rpc_urls,
            default_rpc,
            shutdown: Arc::new(Notify::new()),
        }
    }

    /// Get RPC URL for a specific chain
    fn rpc_for_chain(&self, chain_id: u64) -> &str {
        self.rpc_urls
            .get(&chain_id)
            .map(|s| s.as_str())
            .unwrap_or(&self.default_rpc)
    }

    /// Spawn background polling task.
    pub fn spawn_background_task(self: &Arc<Self>) {
        let this = Arc::clone(self);

        tokio::spawn(async move {
            // Poll every 5 seconds — the tracker itself decides per-tx whether to
            // actually check based on age (fast poll <2min, slow poll after).
            let mut interval = tokio::time::interval(Duration::from_secs(5));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = this.poll_pending_transactions().await {
                            tracing::error!("[tx_tracker] poll error: {}", e);
                        }
                    }
                    _ = this.shutdown.notified() => {
                        tracing::info!("[tx_tracker] shutting down");
                        break;
                    }
                }
            }
        });
    }

    /// Poll all pending/broadcast transactions and check their receipts.
    async fn poll_pending_transactions(&self) -> Result<(), String> {
        // Fetch pending/broadcast transactions that are less than 1 hour old
        let rows: Vec<(Vec<u8>, i64, chrono::DateTime<chrono::Utc>)> = sqlx::query_as(
            "SELECT tx_hash, chain_id, created_at FROM transactions
             WHERE status IN ('pending', 'broadcast')
               AND tx_hash IS NOT NULL
               AND created_at > NOW() - INTERVAL '1 hour'
             ORDER BY created_at ASC
             LIMIT 100"
        )
        .fetch_all(&self.db)
        .await
        .map_err(|e| format!("DB fetch pending txs failed: {}", e))?;

        if rows.is_empty() {
            return Ok(());
        }

        // Mark transactions older than 1 hour as dropped
        let dropped = sqlx::query(
            "UPDATE transactions SET status = 'failed'
             WHERE status IN ('pending', 'broadcast')
               AND created_at <= NOW() - INTERVAL '1 hour'"
        )
        .execute(&self.db)
        .await
        .map_err(|e| format!("DB drop old txs failed: {}", e))?;

        if dropped.rows_affected() > 0 {
            tracing::info!("[tx_tracker] marked {} old transactions as failed (dropped)", dropped.rows_affected());
        }

        let now = chrono::Utc::now();

        for (tx_hash_bytes, chain_id, created_at) in &rows {
            let age = now.signed_duration_since(*created_at);

            // Slow poll: if tx is older than 2 minutes, only check every 30 seconds
            // We achieve this by skipping txs whose age mod 30s is not in the first 5s window
            if age > chrono::Duration::seconds(120) {
                let age_secs = age.num_seconds();
                // Only process on intervals divisible by ~30s (within a 5s window)
                if age_secs % 30 > 5 {
                    continue;
                }
            }

            let tx_hash_hex = format!("0x{}", hex::encode(tx_hash_bytes));
            let rpc_url = self.rpc_for_chain(*chain_id as u64);

            match self.check_receipt(rpc_url, &tx_hash_hex).await {
                Ok(Some(receipt)) => {
                    self.update_confirmed(tx_hash_bytes, &receipt).await;
                }
                Ok(None) => {
                    // Not yet confirmed, keep polling
                }
                Err(e) => {
                    tracing::warn!(
                        "[tx_tracker] receipt check failed for {}: {}",
                        tx_hash_hex, e
                    );
                }
            }
        }

        Ok(())
    }

    /// Call eth_getTransactionReceipt on the given RPC.
    async fn check_receipt(
        &self,
        rpc_url: &str,
        tx_hash: &str,
    ) -> Result<Option<TxReceipt>, String> {
        let rpc_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getTransactionReceipt",
            "params": [tx_hash],
            "id": 1
        });

        let resp = self
            .http
            .post(rpc_url)
            .json(&rpc_body)
            .send()
            .await
            .map_err(|e| format!("RPC request failed: {}", e))?;

        let rpc_resp: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Invalid RPC response: {}", e))?;

        // Check for RPC error
        if let Some(err) = rpc_resp.get("error") {
            let msg = err
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown");
            return Err(format!("RPC error: {}", msg));
        }

        // If result is null, tx is not yet mined
        let result = match rpc_resp.get("result") {
            Some(serde_json::Value::Null) | None => return Ok(None),
            Some(r) => r,
        };

        // Parse receipt fields
        let status_hex = result
            .get("status")
            .and_then(|s| s.as_str())
            .unwrap_or("0x1");
        let success = status_hex == "0x1";

        let block_number_hex = result
            .get("blockNumber")
            .and_then(|b| b.as_str())
            .unwrap_or("0x0");
        let block_number = i64::from_str_radix(
            block_number_hex.strip_prefix("0x").unwrap_or(block_number_hex),
            16,
        )
        .unwrap_or(0);

        let gas_used_hex = result
            .get("gasUsed")
            .and_then(|g| g.as_str())
            .unwrap_or("0x0");
        let gas_used = i64::from_str_radix(
            gas_used_hex.strip_prefix("0x").unwrap_or(gas_used_hex),
            16,
        )
        .unwrap_or(0);

        Ok(Some(TxReceipt {
            success,
            block_number,
            gas_used,
        }))
    }

    /// Update a transaction as confirmed or failed based on receipt.
    async fn update_confirmed(&self, tx_hash_bytes: &[u8], receipt: &TxReceipt) {
        let status = if receipt.success { "confirmed" } else { "failed" };

        let result = sqlx::query(
            "UPDATE transactions
             SET status = $1, block_number = $2, gas_used = $3, confirmed_at = NOW()
             WHERE tx_hash = $4 AND status IN ('pending', 'broadcast')"
        )
        .bind(status)
        .bind(receipt.block_number)
        .bind(receipt.gas_used)
        .bind(tx_hash_bytes)
        .execute(&self.db)
        .await;

        match result {
            Ok(r) if r.rows_affected() > 0 => {
                tracing::info!(
                    "[tx_tracker] tx 0x{} -> {} (block={}, gas={})",
                    hex::encode(tx_hash_bytes),
                    status,
                    receipt.block_number,
                    receipt.gas_used,
                );
            }
            Ok(_) => {} // Already updated or no matching row
            Err(e) => {
                tracing::error!(
                    "[tx_tracker] failed to update tx 0x{}: {}",
                    hex::encode(tx_hash_bytes),
                    e
                );
            }
        }
    }

    /// Signal the background task to stop.
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }
}

/// Parsed transaction receipt data.
struct TxReceipt {
    success: bool,
    block_number: i64,
    gas_used: i64,
}
