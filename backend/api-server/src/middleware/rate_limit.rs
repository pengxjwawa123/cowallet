//! Per-user rate limiting utilities using Redis (production) or in-memory (dev).
//!
//! Limits:
//! - MPC signing endpoints: 10 requests/minute
//! - Read endpoints: 100 requests/minute
//! - Auth endpoints: 5 requests/minute (stricter)

use axum::{
    body::Body,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum::extract::Request;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::Serialize;

/// Rate limit configuration for an endpoint
#[derive(Debug, Clone, Copy)]
pub struct RateLimit {
    pub max_requests: u32,
    pub window_secs: u64,
}

impl RateLimit {
    /// Strict limit for MPC signing operations
    pub const fn strict() -> Self {
        Self {
            max_requests: 10,
            window_secs: 60,
        }
    }

    /// Standard limit for read endpoints
    pub const fn standard() -> Self {
        Self {
            max_requests: 100,
            window_secs: 60,
        }
    }

    /// Very strict limit for auth operations (login, register)
    pub const fn auth() -> Self {
        Self {
            max_requests: 5,
            window_secs: 60,
        }
    }
}

/// Unified rate limiter trait for both Redis and in-memory backends
#[async_trait::async_trait]
pub trait RateLimiter: Clone + Send + Sync + 'static {
    /// Check and record a request atomically
    async fn check_and_record(&self, key: &str, limit: RateLimit) -> RateLimitStatus;
    /// Only check without recording
    async fn check(&self, key: &str, limit: RateLimit) -> RateLimitStatus;
    /// Explicitly record a request
    async fn record(&self, key: &str);
}

/// Rate limited error response
#[derive(Debug, Serialize)]
struct RateLimitError {
    error: &'static str,
    retry_after: u64,
}

/// Result of a rate limit check
#[derive(Debug, Clone)]
pub struct RateLimitStatus {
    pub allowed: bool,
    pub remaining: u32,
    pub retry_after: u64,
}

/// Rate limit rejection response
#[derive(Debug)]
#[allow(dead_code)]
pub enum RateLimitRejection {
    Unauthenticated,
    RateLimited(u64),
}

impl IntoResponse for RateLimitRejection {
    fn into_response(self) -> Response {
        match self {
            RateLimitRejection::Unauthenticated => (
                StatusCode::UNAUTHORIZED,
                "Authentication required for rate limiting",
            )
                .into_response(),
            RateLimitRejection::RateLimited(retry_after) => (
                StatusCode::TOO_MANY_REQUESTS,
                [("Retry-After", retry_after.to_string())],
                axum::Json(RateLimitError {
                    error: "Rate limit exceeded",
                    retry_after,
                }),
            )
                .into_response(),
        }
    }
}

/// Redis-backed rate limiter for production (multi-instance safe)
/// Uses Redis sorted sets with automatic key expiry for sliding window counting
#[derive(Clone)]
pub struct RedisRateLimiter {
    client: redis::Client,
    prefix: String,
}

impl RedisRateLimiter {
    /// Create a new Redis rate limiter with connection string
    pub fn new(redis_url: &str) -> Result<Self, String> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| format!("Failed to create Redis client: {}", e))?;
        Ok(Self {
            client,
            prefix: "ratelimit:".to_string(),
        })
    }

    /// Create from existing Client
    pub fn from_client(client: redis::Client) -> Self {
        Self {
            client,
            prefix: "ratelimit:".to_string(),
        }
    }

    /// Get a connection from the pool
    async fn get_conn(&self) -> Result<MultiplexedConnection, String> {
        self.client
            .get_multiplexed_tokio_connection()
            .await
            .map_err(|e| format!("Redis connection failed: {}", e))
    }
}

#[async_trait::async_trait]
impl RateLimiter for RedisRateLimiter {
    async fn check_and_record(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        let redis_key = format!("{}{}", self.prefix, key);
        let now = std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap_or_default()
            .as_millis() as u64;
        let window_start = now - (limit.window_secs * 1000);

        let mut conn = match self.get_conn().await {
            Ok(c) => c,
            Err(_) => {
                // Fallback: allow on Redis failure (fail-open)
                return RateLimitStatus {
                    allowed: true,
                    remaining: limit.max_requests - 1,
                    retry_after: 0,
                };
            }
        };

        let result: Result<(usize,), redis::RedisError> = redis::pipe()
            .atomic()
            .zrembyscore(&redis_key, 0, window_start)
            .zadd(&redis_key, now, now)
            .ignore()
            .expire(&redis_key, limit.window_secs as i64)
            .ignore()
            .zcard(&redis_key)
            .query_async(&mut conn)
            .await;

        match result {
            Ok((count,)) => {
                if count as u32 <= limit.max_requests {
                    RateLimitStatus {
                        allowed: true,
                        remaining: limit.max_requests.saturating_sub(count as u32),
                        retry_after: 0,
                    }
                } else {
                    // Get earliest timestamp to calculate retry_after
                    let earliest: Result<Vec<u64>, _> = conn.zrange(&redis_key, 0, 0).await;
                    let retry_after = if let Ok(vec) = earliest {
                        if let Some(first) = vec.first() {
                            let elapsed = now - first;
                            limit.window_secs - (elapsed / 1000)
                        } else {
                            limit.window_secs
                        }
                    } else {
                        limit.window_secs
                    };
                    RateLimitStatus {
                        allowed: false,
                        remaining: 0,
                        retry_after,
                    }
                }
            }
            Err(_) => RateLimitStatus {
                allowed: true, // Fail-open for resilience
                remaining: limit.max_requests - 1,
                retry_after: 0,
            },
        }
    }

    async fn check(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        let redis_key = format!("{}{}", self.prefix, key);
        let now = std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap_or_default()
            .as_millis() as u64;
        let window_start = now - (limit.window_secs * 1000);

        let mut conn = match self.get_conn().await {
            Ok(c) => c,
            Err(_) => {
                return RateLimitStatus {
                    allowed: true,
                    remaining: limit.max_requests - 1,
                    retry_after: 0,
                };
            }
        };

        let () = conn.zrembyscore(&redis_key, 0, window_start).await.unwrap_or(());
        let count: usize = conn.zcard(&redis_key).await.unwrap_or(0);

        if count as u32 <= limit.max_requests {
            RateLimitStatus {
                allowed: true,
                remaining: limit.max_requests.saturating_sub(count as u32),
                retry_after: 0,
            }
        } else {
            RateLimitStatus {
                allowed: false,
                remaining: 0,
                retry_after: limit.window_secs,
            }
        }
    }

    async fn record(&self, key: &str) {
        let redis_key = format!("{}{}", self.prefix, key);
        let now = std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap_or_default()
            .as_millis() as u64;

        if let Ok(mut conn) = self.get_conn().await {
            let _: Result<(), redis::RedisError> = conn.zadd(&redis_key, now, now).await;
        }
    }
}

/// In-memory rate limiter (for development/standalone use)
#[derive(Clone)]
pub struct InMemoryRateLimiter {
    limits: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, Vec<std::time::Instant>>>>,
}

impl InMemoryRateLimiter {
    pub fn new() -> Self {
        Self {
            limits: std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashMap::new())),
        }
    }

    pub fn check_rate_limit(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        let mut requests = self.limits.lock().unwrap();
        let timestamps = requests.get(key).map(|v| v.clone()).unwrap_or_default();

        let now = std::time::Instant::now();
        let window_start = now - std::time::Duration::from_secs(limit.window_secs);

        let remaining: Vec<_> = timestamps.iter().copied().filter(|t| t > &window_start).collect();
        let count = remaining.len() as u32;
        if count < limit.max_requests {
            RateLimitStatus {
                allowed: true,
                remaining: limit.max_requests - count - 1,
                retry_after: 0,
            }
        } else {
            let earliest = remaining.first().copied().unwrap_or(now);
            let retry_after = limit.window_secs - now.duration_since(earliest).as_secs();
            RateLimitStatus {
                allowed: false,
                remaining: 0,
                retry_after,
            }
        }
    }

    pub fn record_request(&self, key: String) {
        let mut requests = self.limits.lock().unwrap();
        let timestamps = requests.entry(key).or_insert_with(Vec::new);
        timestamps.push(std::time::Instant::now());
    }
}

#[async_trait::async_trait]
impl RateLimiter for InMemoryRateLimiter {
    async fn check_and_record(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        let status = self.check_rate_limit(key, limit);
        if status.allowed {
            self.record_request(key.to_string());
        }
        status
    }

    async fn check(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        self.check_rate_limit(key, limit)
    }

    async fn record(&self, key: &str) {
        self.record_request(key.to_string());
    }
}

impl Default for InMemoryRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

/// Wrapper enum for runtime selection of rate limiter backend
#[derive(Clone)]
pub enum AnyRateLimiter {
    InMemory(InMemoryRateLimiter),
    Redis(RedisRateLimiter),
}

impl AnyRateLimiter {
    /// Create from environment - uses Redis if REDIS_URL set, otherwise in-memory
    pub fn from_env() -> Result<Self, String> {
        match std::env::var("REDIS_URL") {
            Ok(url) => Ok(Self::Redis(RedisRateLimiter::new(&url)?)),
            Err(_) => {
                tracing::warn!("REDIS_URL not set, using in-memory rate limiter (dev only)");
                Ok(Self::InMemory(InMemoryRateLimiter::new()))
            }
        }
    }

    /// Force in-memory mode
    pub fn in_memory() -> Self {
        Self::InMemory(InMemoryRateLimiter::new())
    }
}

#[async_trait::async_trait]
impl RateLimiter for AnyRateLimiter {
    async fn check_and_record(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        match self {
            AnyRateLimiter::InMemory(l) => l.check_and_record(key, limit).await,
            AnyRateLimiter::Redis(l) => l.check_and_record(key, limit).await,
        }
    }

    async fn check(&self, key: &str, limit: RateLimit) -> RateLimitStatus {
        match self {
            AnyRateLimiter::InMemory(l) => l.check(key, limit).await,
            AnyRateLimiter::Redis(l) => l.check(key, limit).await,
        }
    }

    async fn record(&self, key: &str) {
        match self {
            AnyRateLimiter::InMemory(l) => l.record(key).await,
            AnyRateLimiter::Redis(l) => l.record(key).await,
        }
    }
}

use crate::state::AppState;

/// Axum middleware for rate limiting (strict: 10 req/min)
pub async fn strict_rate_limit_middleware(request: Request<Body>, next: Next) -> Response {
    apply_rate_limit(request, next, RateLimit::strict()).await
}

/// Axum middleware for rate limiting (standard: 100 req/min)
pub async fn standard_rate_limit_middleware(request: Request<Body>, next: Next) -> Response {
    apply_rate_limit(request, next, RateLimit::standard()).await
}

/// Axum middleware for rate limiting (auth: 5 req/min)
pub async fn auth_rate_limit_middleware(request: Request<Body>, next: Next) -> Response {
    apply_rate_limit(request, next, RateLimit::auth()).await
}

/// Internal rate limiting implementation
async fn apply_rate_limit(mut request: Request<Body>, next: Next, limit: RateLimit) -> Response {
    // Extract AppState from request extensions
    let state = match request.extensions().get::<AppState>() {
        Some(s) => s.clone(),
        None => {
            tracing::warn!("Rate limiting: AppState not found in extensions, allowing request");
            return next.run(request).await;
        }
    };

    // Try to get user ID from claims if authenticated, or use IP as fallback
    let key = request
        .extensions()
        .get::<crate::middleware::auth::Claims>()
        .map(|c| format!("user:{}", c.sub))
        .or_else(|| {
            request
                .headers()
                .get("X-Forwarded-For")
                .and_then(|h| h.to_str().ok())
                .map(|s| format!("ip:{}", s.split(',').next().unwrap_or(s).trim()))
        })
        .or_else(|| {
            request
                .extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|c| format!("ip:{}", c.ip()))
        })
        .unwrap_or_else(|| "unknown".to_string());

    // Check and record rate limit
    let status = state.rate_limiter.check_and_record(&key, limit).await;

    if !status.allowed {
        tracing::warn!("Rate limit exceeded for {}: {} requests/{}s", key, limit.max_requests, limit.window_secs);
        return RateLimitRejection::RateLimited(status.retry_after).into_response();
    }

    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rate_limit_check() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 3,
            window_secs: 60,
        };

        // First 3 requests should be allowed
        for i in 0..3 {
            let status = limiter.check_rate_limit("test_user", limit);
            assert!(status.allowed, "Request {} should be allowed", i + 1);
            limiter.record_request("test_user".to_string());
        }

        // 4th request should be denied
        let status = limiter.check_rate_limit("test_user", limit);
        assert!(!status.allowed, "4th request should be denied");
        assert!(status.retry_after > 0);
    }

    #[test]
    fn test_rate_limit_different_keys() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 2,
            window_secs: 60,
        };

        // User 1 uses up their quota (2 requests)
        limiter.record_request("user1".to_string());
        limiter.record_request("user1".to_string());

        // User 2 should still have quota
        let status = limiter.check_rate_limit("user2", limit);
        assert!(status.allowed);

        // User 1 should be rate limited (3rd request not allowed)
        let status = limiter.check_rate_limit("user1", limit);
        assert!(!status.allowed);
    }

    #[test]
    fn test_rate_limiter_allows_within_limit() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 5,
            window_secs: 60,
        };

        // All requests within limit should be allowed
        for i in 0..5 {
            let status = limiter.check_rate_limit("user_123", limit);
            assert!(status.allowed, "Request {} should be allowed", i + 1);
            assert_eq!(status.remaining, 5 - i - 1);
            limiter.record_request("user_123".to_string());
        }
    }

    #[test]
    fn test_rate_limiter_blocks_over_limit() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 3,
            window_secs: 60,
        };

        // Use up the quota
        for _ in 0..3 {
            let status = limiter.check_rate_limit("user_456", limit);
            assert!(status.allowed);
            limiter.record_request("user_456".to_string());
        }

        // Next request should be blocked
        let status = limiter.check_rate_limit("user_456", limit);
        assert!(!status.allowed);
        assert_eq!(status.remaining, 0);
        assert!(status.retry_after > 0);
        assert!(status.retry_after <= limit.window_secs);
    }

    #[test]
    fn test_rate_limit_presets() {
        let strict = RateLimit::strict();
        assert_eq!(strict.max_requests, 10);
        assert_eq!(strict.window_secs, 60);

        let standard = RateLimit::standard();
        assert_eq!(standard.max_requests, 100);
        assert_eq!(standard.window_secs, 60);

        let auth = RateLimit::auth();
        assert_eq!(auth.max_requests, 5);
        assert_eq!(auth.window_secs, 60);
    }

    #[tokio::test]
    async fn test_in_memory_rate_limiter_trait_check_and_record() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 2,
            window_secs: 60,
        };

        // First request should succeed
        let status = limiter.check_and_record("test_key", limit).await;
        assert!(status.allowed);
        assert_eq!(status.remaining, 1);

        // Second request should succeed
        let status = limiter.check_and_record("test_key", limit).await;
        assert!(status.allowed);
        assert_eq!(status.remaining, 0);

        // Third request should be blocked
        let status = limiter.check_and_record("test_key", limit).await;
        assert!(!status.allowed);
        assert_eq!(status.remaining, 0);
    }

    #[tokio::test]
    async fn test_in_memory_rate_limiter_trait_check_without_record() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 2,
            window_secs: 60,
        };

        // Check without recording should not consume quota
        let status = limiter.check("test_check", limit).await;
        assert!(status.allowed);

        let status = limiter.check("test_check", limit).await;
        assert!(status.allowed);

        // Should still have full quota
        let status = limiter.check_and_record("test_check", limit).await;
        assert!(status.allowed);
        assert_eq!(status.remaining, 1);
    }

    #[tokio::test]
    async fn test_in_memory_rate_limiter_trait_record() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 2,
            window_secs: 60,
        };

        // Explicitly record requests
        limiter.record("test_record").await;
        limiter.record("test_record").await;

        // Should be at limit
        let status = limiter.check("test_record", limit).await;
        assert!(!status.allowed);
    }

    #[test]
    fn test_rate_limit_isolation_between_keys() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 1,
            window_secs: 60,
        };

        // Max out user1
        limiter.record_request("user1".to_string());
        let status = limiter.check_rate_limit("user1", limit);
        assert!(!status.allowed);

        // user2 should be unaffected
        let status = limiter.check_rate_limit("user2", limit);
        assert!(status.allowed);

        // user3 should be unaffected
        let status = limiter.check_rate_limit("user3", limit);
        assert!(status.allowed);
    }

    #[test]
    fn test_rate_limit_window_cleanup() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 2,
            window_secs: 1, // 1 second window
        };

        // Use up quota
        limiter.record_request("user_window".to_string());
        limiter.record_request("user_window".to_string());

        // Should be blocked
        let status = limiter.check_rate_limit("user_window", limit);
        assert!(!status.allowed);

        // Wait for window to expire
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Should be allowed again (old requests outside window)
        let status = limiter.check_rate_limit("user_window", limit);
        assert!(status.allowed);
    }

    #[test]
    fn test_any_rate_limiter_in_memory_mode() {
        let limiter = AnyRateLimiter::in_memory();

        match limiter {
            AnyRateLimiter::InMemory(_) => { /* expected */ }
            AnyRateLimiter::Redis(_) => panic!("Expected InMemory variant"),
        }
    }

    #[test]
    fn test_rate_limit_error_response_format() {
        let rejection = RateLimitRejection::RateLimited(60);
        let response = rejection.into_response();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);

        // Check retry-after header
        let headers = response.headers();
        let retry_after = headers.get("Retry-After");
        assert!(retry_after.is_some());
    }

    #[test]
    fn test_rate_limit_unauthenticated_response() {
        let rejection = RateLimitRejection::Unauthenticated;
        let response = rejection.into_response();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_rate_limiter_remaining_count_accuracy() {
        let limiter = InMemoryRateLimiter::new();
        let limit = RateLimit {
            max_requests: 5,
            window_secs: 60,
        };

        // Check remaining count decreases correctly
        for i in 0..5 {
            let status = limiter.check_and_record("accuracy_test", limit).await;
            assert!(status.allowed);
            assert_eq!(status.remaining, 5 - i - 1, "Remaining count mismatch at iteration {}", i);
        }

        // Over limit should have 0 remaining
        let status = limiter.check("accuracy_test", limit).await;
        assert!(!status.allowed);
        assert_eq!(status.remaining, 0);
    }
}
