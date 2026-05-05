use sqlx::PgPool;

use crate::middleware::audit::AuditLogger;
use crate::middleware::metrics::MetricsStore;
use crate::middleware::rate_limit::AnyRateLimiter;
use crate::retry::{CircuitBreaker, CircuitBreakerConfig};
use crate::routes::price::PriceCache;
use crate::routes::yield_::YieldCache;
use crate::services::claude::ClaudeClient;

#[derive(Clone)]
pub struct AppState {
    pub db: Option<PgPool>,
    pub rpc_url: String,
    pub price_cache: PriceCache,
    pub yield_cache: YieldCache,
    pub http: reqwest::Client,
    pub claude: Option<ClaudeClient>,
    pub rate_limiter: AnyRateLimiter,
    pub rpc_circuit_breaker: CircuitBreaker,
    pub defi_circuit_breaker: CircuitBreaker,
    pub metrics: MetricsStore,
    pub audit_logger: AuditLogger,
}

impl AppState {
    pub async fn new(database_url: &str, rpc_url: String) -> Result<Self, sqlx::Error> {
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

        // Initialize Claude client if API key is available
        let claude = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .and_then(|key| match ClaudeClient::new(key) {
                Ok(client) => Some(client),
                Err(e) => {
                    tracing::warn!("Failed to initialize Claude client: {}", e);
                    None
                }
            });

        Ok(Self {
            db: Some(db.clone()),
            rpc_url,
            price_cache: PriceCache::new(),
            yield_cache: YieldCache::new(),
            http: Self::create_http_client(),
            claude,
            rate_limiter: AnyRateLimiter::from_env().unwrap_or_else(|_| AnyRateLimiter::in_memory()),
            rpc_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            defi_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            metrics: MetricsStore::new(),
            audit_logger: AuditLogger::new(Some(db)),
        })
    }

    pub fn without_db() -> Self {
        let claude = std::env::var("ANTHROPIC_API_KEY")
            .ok()
            .and_then(|key| match ClaudeClient::new(key) {
                Ok(client) => Some(client),
                Err(e) => {
                    tracing::warn!("Failed to initialize Claude client: {}", e);
                    None
                }
            });

        Self {
            db: None,
            rpc_url: "https://sepolia.base.org".into(),
            price_cache: PriceCache::new(),
            yield_cache: YieldCache::new(),
            http: Self::create_http_client(),
            claude,
            rate_limiter: AnyRateLimiter::from_env().unwrap_or_else(|_| AnyRateLimiter::in_memory()),
            rpc_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            defi_circuit_breaker: CircuitBreaker::new(CircuitBreakerConfig::default()),
            metrics: MetricsStore::new(),
            audit_logger: AuditLogger::noop(),
        }
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
