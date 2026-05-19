use std::collections::HashMap;
use std::sync::Arc;
use sqlx::PgPool;

use crate::middleware::audit::AuditLogger;
use crate::middleware::metrics::MetricsStore;
use crate::middleware::rate_limit::AnyRateLimiter;
use crate::retry::{CircuitBreaker, CircuitBreakerConfig};
use crate::routes::price::PriceCache;
use crate::routes::yield_::YieldCache;
use crate::services::claude::AiClient;
// use crate::services::email::EmailService; // uncomment after aws-sdk-sesv2
use crate::services::mpc_participant::MpcParticipant;
use crate::services::presign_manager::PresignManager;
use crate::services::tx_tracker::TxTracker;

#[derive(Clone)]
pub struct AppState {
    pub db: Option<PgPool>,
    pub rpc_url: String,
    pub rpc_urls: HashMap<u64, String>,
    pub price_cache: PriceCache,
    pub yield_cache: YieldCache,
    pub http: reqwest::Client,
    pub claude: Option<AiClient>,
    pub nats: Option<async_nats::Client>,
    pub rate_limiter: AnyRateLimiter,
    pub rpc_circuit_breaker: CircuitBreaker,
    pub defi_circuit_breaker: CircuitBreaker,
    pub metrics: MetricsStore,
    pub audit_logger: AuditLogger,
    pub mpc_participant: Option<Arc<MpcParticipant>>,
    pub presign_manager: Option<Arc<PresignManager>>,
    pub covalent_api_key: Option<String>,
    pub zerox_api_key: Option<String>,
    pub bundler_url: Option<String>,
    pub paymaster_url: Option<String>,
    pub tx_tracker: Option<Arc<TxTracker>>,
    // pub email: Option<EmailService>, // uncomment after aws-sdk-sesv2
}

impl AppState {
    pub async fn new(database_url: &str, rpc_url: String, rpc_urls: HashMap<u64, String>) -> Result<Self, sqlx::Error> {
        // Configure production-grade connection pool
        let pool_options = sqlx::postgres::PgPoolOptions::new()
            .max_connections(
                std::env::var("DB_MAX_CONNECTIONS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(20),
            )
            .min_connections(
                std::env::var("DB_MIN_CONNECTIONS")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(5),
            )
            .acquire_timeout(std::time::Duration::from_secs(
                std::env::var("DB_ACQUIRE_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(10),
            ))
            .idle_timeout(std::time::Duration::from_secs(
                std::env::var("DB_IDLE_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(600),
            ))
            .max_lifetime(std::time::Duration::from_secs(
                std::env::var("DB_MAX_LIFETIME")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1800),
            ));

        let db = pool_options.connect(database_url).await?;
        sqlx::migrate!("../migrations").run(&db).await?;

        // Initialize NATS client if URL is available
        let nats = match std::env::var("NATS_URL") {
            Ok(url) => {
                match async_nats::connect(&url).await {
                    Ok(client) => {
                        tracing::info!("Connected to NATS at {}", url);
                        Some(client)
                    }
                    Err(e) => {
                        tracing::warn!("Failed to connect to NATS at {}: {} — WS will fall back to DB polling", url, e);
                        None
                    }
                }
            }
            Err(_) => {
                tracing::info!("NATS_URL not set — MPC WebSocket will use DB polling fallback");
                None
            }
        };

        // Initialize AI client (DeepSeek)
        let claude = match AiClient::from_env() {
            Ok(client) => Some(client),
            Err(e) => {
                tracing::warn!("AI client not configured: {}", e);
                None
            }
        };

        // Initialize MPC participant with encryption service (ENCRYPTION_KEY validated in main)
        let encryption_key = hex::decode(
            std::env::var("ENCRYPTION_KEY").expect("ENCRYPTION_KEY must be set")
        ).expect("ENCRYPTION_KEY must be valid hex");
        let mut key_array = [0u8; 32];
        key_array.copy_from_slice(&encryption_key);
        let encryption = crate::services::crypto::EncryptionService::new(&key_array, "server-mpc");

        // Initialize PresignManager with encryption service
        let presign_manager = Arc::new(PresignManager::new(db.clone(), encryption.clone()));
        let min_presignatures: u32 = std::env::var("PRESIGN_MIN_COUNT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5);
        presign_manager.spawn_background_task(min_presignatures);

        // Initialize MPC participant with encryption service and presign manager
        let mut participant = MpcParticipant::new(db.clone(), encryption);
        participant.set_presign_manager(Arc::clone(&presign_manager));
        let participant = Arc::new(participant);
        participant.spawn_cleanup();

        let covalent_api_key = std::env::var("COVALENT_API_KEY")
            .ok()
            .filter(|s| !s.is_empty())
            .or_else(|| Some("cqt_rQT8rPY3Bx6R7HWj6wGHCWFh4F6K".to_string()));
        if covalent_api_key.is_some() {
            tracing::info!("Covalent API configured for balance queries");
        } else {
            tracing::warn!("COVALENT_API_KEY not set — balance and tx-history endpoints will return 503");
        }

        let zerox_api_key = std::env::var("ZEROX_API_KEY")
            .ok()
            .filter(|s| !s.is_empty());
        if zerox_api_key.is_some() {
            tracing::info!("0x API key configured for DEX swaps");
        } else {
            tracing::info!("ZEROX_API_KEY not set — swap quotes will use free tier (rate limited)");
        }

        let bundler_url = std::env::var("BUNDLER_URL")
            .ok()
            .filter(|s| !s.is_empty());
        if let Some(ref url) = bundler_url {
            tracing::info!("Bundler configured at {}", url);
        } else {
            tracing::info!("BUNDLER_URL not set — ERC-4337 account abstraction disabled");
        }

        let paymaster_url = std::env::var("PAYMASTER_URL")
            .ok()
            .filter(|s| !s.is_empty());
        if let Some(ref url) = paymaster_url {
            tracing::info!("Paymaster configured at {}", url);
        }

        // Initialize transaction confirmation tracker
        let http_client = Self::create_http_client();
        let tx_tracker = Arc::new(TxTracker::new(
            db.clone(),
            http_client.clone(),
            rpc_urls.clone(),
            rpc_url.clone(),
        ));
        tx_tracker.spawn_background_task();
        tracing::info!("Transaction confirmation tracker started");

        Ok(Self {
            db: Some(db.clone()),
            rpc_url,
            rpc_urls,
            price_cache: PriceCache::new(),
            yield_cache: YieldCache::new(),
            http: http_client,
            claude,
            nats,
            rate_limiter: AnyRateLimiter::from_env().unwrap_or_else(|_| AnyRateLimiter::in_memory()),
            rpc_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            defi_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            metrics: MetricsStore::new(),
            audit_logger: AuditLogger::new(Some(db)),
            mpc_participant: Some(participant),
            presign_manager: Some(presign_manager),
            covalent_api_key,
            zerox_api_key,
            bundler_url,
            paymaster_url,
            tx_tracker: Some(tx_tracker),
            // email: EmailService::from_env().await, // uncomment after aws-sdk-sesv2
        })
    }

    /// Get RPC URL for a specific chain, with fallback to default
    pub fn rpc_for_chain(&self, chain_id: u64) -> &str {
        self.rpc_urls
            .get(&chain_id)
            .map(|s| s.as_str())
            .unwrap_or(&self.rpc_url)
    }


    /// Create a production-grade HTTP client with reasonable defaults
    fn create_http_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .build()
            .unwrap_or_default()
    }

    /// Check if database connection is available - returns production error
    pub fn require_db(&self) -> crate::errors::Result<&PgPool> {
        self.db
            .as_ref()
            .ok_or_else(|| crate::errors::ApiError::service_unavailable("Database unavailable"))
    }
}
