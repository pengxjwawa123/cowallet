use tracing_subscriber;

/// EVM chain event indexer.
///
/// Subscribes to blockchain events for tracked addresses and indexes
/// transactions into PostgreSQL for history queries.
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("cowallet indexer starting");

    // TODO:
    // 1. Connect to EVM RPC (WebSocket for real-time)
    // 2. Subscribe to Transfer events for tracked addresses
    // 3. Parse and index transactions into PostgreSQL
    // 4. Handle chain reorgs (delete + re-index)
    // 5. Multi-chain: run one subscriber per chain

    tracing::info!("indexer ready");
    tokio::signal::ctrl_c().await.unwrap();
}
