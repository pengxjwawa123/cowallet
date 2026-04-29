use tracing_subscriber;

/// Background worker for periodic tasks.
///
/// Jobs:
/// - Key resharing scheduler (every 30 days per user)
/// - Price feed updater (every 30 seconds)
/// - Expired session cleanup
/// - Approval request expiry check
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("cowallet worker starting");

    // TODO:
    // 1. Connect to PostgreSQL + Redis
    // 2. Spawn periodic tasks:
    //    - reshare_scheduler: check which users need key refresh
    //    - price_updater: fetch CoinGecko → Redis cache
    //    - session_cleanup: remove expired MPC sessions
    //    - approval_expiry: expire old approval requests

    tracing::info!("worker ready");
    tokio::signal::ctrl_c().await.unwrap();
}
