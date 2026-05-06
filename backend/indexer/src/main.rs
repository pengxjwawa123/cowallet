//! EVM chain event indexer.
//!
//! Subscribes to blockchain events for tracked addresses and indexes
//! transactions into PostgreSQL for history queries.

use alloy_primitives::{Address, B256, U256};
use alloy_provider::Provider;
use alloy_rpc_types::Filter;
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::collections::HashSet;
use std::time::Duration;
use tracing_subscriber;

/// Configuration for the indexer
#[derive(Debug, Clone)]
pub struct IndexerConfig {
    pub rpc_ws_url: String,
    pub chain_id: u64,
    pub chain_name: String,
    pub poll_interval: Duration,
    pub start_block: Option<u64>,
}

impl Default for IndexerConfig {
    fn default() -> Self {
        Self {
            rpc_ws_url: "ws://localhost:8545".into(),
            chain_id: 1,
            chain_name: "ethereum".into(),
            poll_interval: Duration::from_secs(12),
            start_block: None,
        }
    }
}

/// Indexer state and connections
pub struct ChainIndexer {
    config: IndexerConfig,
    db: PgPool,
    tracked_addresses: HashSet<Address>,
    last_processed_block: u64,
}

impl ChainIndexer {
    /// Create a new indexer with database connection
    pub async fn new(config: IndexerConfig, db_url: &str) -> Result<Self, IndexerError> {
        let db = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(5))
            .connect(db_url)
            .await?;

        // Load last processed block from database
        let last_block: Option<(i64,)> = sqlx::query_as(
            "SELECT block_number FROM indexer_state WHERE chain_id = $1 ORDER BY block_number DESC LIMIT 1"
        )
        .bind(config.chain_id as i64)
        .fetch_optional(&db)
        .await?;

        let last_processed_block = last_block
            .map(|(b,)| b as u64)
            .unwrap_or(config.start_block.unwrap_or(0));

        Ok(Self {
            config,
            db,
            tracked_addresses: HashSet::new(),
            last_processed_block,
        })
    }

    /// Add an address to track for transfers
    pub fn track_address(&mut self, address: Address) {
        self.tracked_addresses.insert(address);
        tracing::info!("Now tracking address: {:?}", address);
    }

    /// Remove an address from tracking
    pub fn untrack_address(&mut self, address: &Address) {
        self.tracked_addresses.remove(address);
        tracing::info!("Stopped tracking address: {:?}", address);
    }

    /// Fetch and index blocks from start_block to current head
    pub async fn catchup_blocks(&mut self) -> Result<(), IndexerError> {
        let provider = alloy_provider::ProviderBuilder::new()
            .connect_http(self.config.rpc_ws_url.parse().expect("invalid RPC URL"));

        let current_block = provider.get_block_number().await
            .map_err(|e| IndexerError::Rpc(e.to_string()))?;
        tracing::info!(
            "Catching up from block {} to {} on chain {}",
            self.last_processed_block,
            current_block,
            self.config.chain_id
        );

        // Process in batches of 100 blocks to avoid memory issues
        let batch_size = 100u64;
        let mut from_block = self.last_processed_block + 1;

        while from_block <= current_block {
            let to_block = std::cmp::min(from_block + batch_size - 1, current_block);

            self.process_block_range(&provider, from_block, to_block).await
                .map_err(|e| IndexerError::Rpc(e.to_string()))?;

            // Update state after each batch
            self.update_last_processed_block(to_block).await?;
            tracing::debug!("Processed blocks {} - {}", from_block, to_block);

            from_block = to_block + 1;
        }

        tracing::info!("Catchup complete. Last processed block: {}", current_block);
        Ok(())
    }

    /// Process a range of blocks for Transfer events
    async fn process_block_range<P: Provider>(
        &mut self,
        provider: &P,
        from_block: u64,
        to_block: u64,
    ) -> Result<(), IndexerError> {
        // Build Transfer event filter (ERC-20 Transfer signature)
        let transfer_signature = alloy_primitives::b256!(
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a1fc74ea488187093022"
        );

        let filter = Filter::new()
            .event_signature(transfer_signature)
            .from_block(from_block)
            .to_block(to_block);

        let logs = provider.get_logs(&filter).await
            .map_err(|e| IndexerError::Rpc(e.to_string()))?;

        for log in logs {
            self.process_transfer_log(log).await?;
        }

        Ok(())
    }

    /// Process a single Transfer log entry
    async fn process_transfer_log(&mut self, log: alloy_rpc_types::Log) -> Result<(), IndexerError> {
        // Extract from, to, value from log topics and data
        // Transfer topics: [signature, from, to]
        if log.topics().len() < 3 {
            return Ok(()); // Not a standard Transfer event
        }

        let from = Address::from_slice(&log.topics()[1][12..]);
        let to = Address::from_slice(&log.topics()[2][12..]);

        // Check if this transfer involves any tracked address
        if !self.tracked_addresses.contains(&from) && !self.tracked_addresses.contains(&to) {
            return Ok(());
        }

        // Parse value from data (first 32 bytes)
        let log_data = log.data();
        let value = if log_data.data.len() >= 32 {
            U256::from_be_bytes::<32>(log_data.data[0..32].try_into().unwrap_or([0; 32]))
        } else {
            U256::ZERO
        };

        let block_number = log.block_number.unwrap_or(0);
        let tx_hash = log.transaction_hash.unwrap_or_default();

        tracing::debug!(
            "Found Transfer: {} -> {} value={} tx={}",
            from, to, value, tx_hash
        );

        // Upsert transaction into database
        self.upsert_transaction(
            log.address(), // Token contract (or zero for native)
            from,
            to,
            value,
            block_number,
            tx_hash,
            log.log_index.unwrap_or(0) as u32,
        ).await?;

        Ok(())
    }

    /// Insert or update a transaction in the database
    async fn upsert_transaction(
        &self,
        token_address: Address,
        from: Address,
        to: Address,
        value: U256,
        block_number: u64,
        tx_hash: B256,
        log_index: u32,
    ) -> Result<(), IndexerError> {
        let value_str = value.to_string();

        sqlx::query(
            r#"
            INSERT INTO transactions
                (chain_id, from_addr, to_addr, value, token_address, tx_hash,
                 block_number, log_index, status)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'confirmed')
            ON CONFLICT (tx_hash, log_index) DO UPDATE SET
                block_number = EXCLUDED.block_number
            "#,
        )
        .bind(self.config.chain_id as i64)
        .bind(from.as_slice())
        .bind(to.as_slice())
        .bind(value_str)
        .bind(token_address.as_slice())
        .bind(tx_hash.as_slice())
        .bind(block_number as i64)
        .bind(log_index as i64)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Update last processed block in database
    async fn update_last_processed_block(&self, block_number: u64) -> Result<(), IndexerError> {
        sqlx::query(
            r#"
            INSERT INTO indexer_state (chain_id, block_number, updated_at)
            VALUES ($1, $2, NOW())
            ON CONFLICT (chain_id) DO UPDATE SET
                block_number = EXCLUDED.block_number,
                updated_at = NOW()
            "#,
        )
        .bind(self.config.chain_id as i64)
        .bind(block_number as i64)
        .execute(&self.db)
        .await?;

        Ok(())
    }

    /// Run the main indexer loop with WebSocket subscription
    pub async fn run(mut self) -> Result<(), IndexerError> {
        tracing::info!(
            "Starting indexer for chain {} ({})",
            self.config.chain_name,
            self.config.chain_id
        );

        // First catch up with historical blocks
        if self.config.start_block.is_some() {
            self.catchup_blocks().await?;
        }

        // Start polling loop for new blocks
        let mut interval = tokio::time::interval(self.config.poll_interval);

        loop {
            interval.tick().await;

            if let Err(e) = self.poll_new_blocks().await {
                tracing::error!("Error polling new blocks: {}", e);
            }
        }
    }

    /// Poll for new blocks and process them (batched to respect RPC limits)
    async fn poll_new_blocks(&mut self) -> Result<(), IndexerError> {
        let provider = alloy_provider::ProviderBuilder::new()
            .connect_http(self.config.rpc_ws_url.parse().expect("invalid RPC URL"));

        let current_block = provider.get_block_number().await
            .map_err(|e| IndexerError::Rpc(e.to_string()))?;

        if current_block <= self.last_processed_block {
            return Ok(());
        }

        let batch_size = 2000u64;
        let mut from_block = self.last_processed_block + 1;

        while from_block <= current_block {
            let to_block = std::cmp::min(from_block + batch_size - 1, current_block);

            self.process_block_range(&provider, from_block, to_block).await
                .map_err(|e| IndexerError::Rpc(e.to_string()))?;

            self.update_last_processed_block(to_block).await?;
            self.last_processed_block = to_block;
            tracing::debug!("Processed blocks {} - {}", from_block, to_block);

            from_block = to_block + 1;
        }

        Ok(())
    }
}

/// Indexer error type
#[derive(Debug, thiserror::Error)]
pub enum IndexerError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("RPC/provider error: {0}")]
    Rpc(String),

    #[error("chain reorg detected at block {0}")]
    Reorg(u64),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[tokio::main]
async fn main() -> Result<(), IndexerError> {
    tracing_subscriber::fmt::init();
    tracing::info!("cowallet indexer starting");

    // Load configuration from environment
    let rpc_ws_url = std::env::var("RPC_WS_URL")
        .unwrap_or_else(|_| "ws://localhost:8545".into());
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://localhost/cowallet".into());
    let chain_id: u64 = std::env::var("CHAIN_ID")
        .unwrap_or_else(|_| "1".into())
        .parse()
        .unwrap_or(1);
    let chain_name = std::env::var("CHAIN_NAME")
        .unwrap_or_else(|_| "ethereum".into());

    let config = IndexerConfig {
        rpc_ws_url,
        chain_id,
        chain_name,
        ..Default::default()
    };

    let indexer = ChainIndexer::new(config, &db_url).await?;

    tracing::info!("indexer ready");
    indexer.run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = IndexerConfig::default();
        assert_eq!(config.chain_id, 1);
        assert_eq!(config.chain_name, "ethereum");
    }
}
