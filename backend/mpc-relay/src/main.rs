use tracing_subscriber;

/// MPC message relay service.
///
/// Connects to NATS and routes protocol messages between MPC parties.
/// Each signing session gets a dedicated NATS subject:
///   cowallet.mpc.{session_id}.{party_index}
#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("cowallet MPC relay starting");

    // TODO:
    // 1. Connect to NATS
    // 2. Subscribe to cowallet.mpc.>
    // 3. Route messages between parties
    // 4. Manage session lifecycle (create, timeout, cleanup)
    // 5. Enforce authentication on session creation

    tracing::info!("MPC relay ready");
    tokio::signal::ctrl_c().await.unwrap();
}
