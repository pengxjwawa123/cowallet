//! Retry and circuit breaker patterns for external service calls
//!
//! Provides:
//! - Exponential backoff with jitter
//! - Circuit breaker pattern for failing services
//! - Configurable retry policies

use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::warn;

/// Configuration for retry behavior
#[derive(Debug, Clone, Copy)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Initial delay between retries
    pub initial_delay: Duration,
    /// Maximum delay between retries
    pub max_delay: Duration,
    /// Exponential backoff multiplier
    pub backoff_multiplier: f64,
    /// Jitter factor (0.0 = no jitter, 1.0 = full jitter)
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
            backoff_multiplier: 2.0,
            jitter_factor: 0.5,
        }
    }
}

impl RetryConfig {
    /// Aggressive retry policy for transient errors
    pub fn aggressive() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_millis(50),
            max_delay: Duration::from_secs(10),
            backoff_multiplier: 2.0,
            jitter_factor: 0.5,
        }
    }

    /// Conservative retry policy for expensive operations
    pub fn conservative() -> Self {
        Self {
            max_retries: 2,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(2),
            backoff_multiplier: 1.5,
            jitter_factor: 0.3,
        }
    }

    /// Calculate delay for a specific retry attempt
    fn calculate_delay(&self, attempt: u32) -> Duration {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        let base_delay = self.initial_delay.as_millis() as f64
            * self.backoff_multiplier.powf(attempt as f64);

        let capped_delay = base_delay.min(self.max_delay.as_millis() as f64);

        // Add jitter
        let jitter_range = capped_delay * self.jitter_factor;
        let jitter: f64 = rng.sample(rand::distributions::Standard);
        let jitter = jitter * jitter_range;
        let final_delay = capped_delay - (jitter_range / 2.0) + jitter;

        Duration::from_millis(final_delay.max(1.0) as u64)
    }
}

/// Retry an operation with exponential backoff and jitter
pub async fn retry_with_backoff<F, Fut, T, E, ShouldRetryFn>(
    config: RetryConfig,
    mut operation: F,
    mut should_retry: ShouldRetryFn,
    operation_name: &str,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    ShouldRetryFn: FnMut(&E) -> bool,
{
    let mut attempt = 0;

    loop {
        match operation().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                attempt += 1;

                if attempt >= config.max_retries || !should_retry(&err) {
                    return Err(err);
                }

                let delay = config.calculate_delay(attempt);
                warn!(
                    "{} failed, attempt {}/{}, retrying in {:?}",
                    operation_name, attempt, config.max_retries, delay
                );

                tokio::time::sleep(delay).await;
            }
        }
    }
}

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation
    Closed,
    /// Failing fast, no calls allowed
    Open,
    /// Testing with limited calls
    HalfOpen,
}

/// Circuit breaker for preventing cascading failures
#[derive(Clone)]
pub struct CircuitBreaker {
    state: Arc<RwLock<CircuitState>>,
    failure_count: Arc<RwLock<u32>>,
    success_count: Arc<RwLock<u32>>,
    last_failure: Arc<RwLock<std::time::Instant>>,
    config: CircuitBreakerConfig,
}

/// Configuration for circuit breaker
#[derive(Debug, Clone, Copy)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening circuit
    pub failure_threshold: u32,
    /// Time to wait before attempting recovery
    pub recovery_timeout: Duration,
    /// Number of successful tests before closing circuit
    pub success_threshold: u32,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        }
    }
}

impl CircuitBreaker {
    /// Create a new circuit breaker
    pub fn new(config: CircuitBreakerConfig) -> Self {
        Self {
            state: Arc::new(RwLock::new(CircuitState::Closed)),
            failure_count: Arc::new(RwLock::new(0)),
            success_count: Arc::new(RwLock::new(0)),
            last_failure: Arc::new(RwLock::new(std::time::Instant::now())),
            config,
        }
    }

    /// Check if the circuit is currently allowing calls
    pub async fn is_allowed(&self) -> bool {
        let state = *self.state.read().await;

        match state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                let last_failure = *self.last_failure.read().await;
                if last_failure.elapsed() >= self.config.recovery_timeout {
                    // Transition to half-open for recovery test
                    let mut state = self.state.write().await;
                    *state = CircuitState::HalfOpen;
                    *self.success_count.write().await = 0;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,
        }
    }

    /// Record a successful call
    pub async fn record_success(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                // Reset failure count on success
                *self.failure_count.write().await = 0;
            }
            CircuitState::HalfOpen => {
                let mut successes = self.success_count.write().await;
                *successes += 1;
                if *successes >= self.config.success_threshold {
                    // Circuit recovered
                    *state = CircuitState::Closed;
                    *self.failure_count.write().await = 0;
                    tracing::info!("Circuit breaker closed - service recovered");
                }
            }
            CircuitState::Open => {}
        }
    }

    /// Record a failed call
    pub async fn record_failure(&self) {
        let mut state = self.state.write().await;

        match *state {
            CircuitState::Closed => {
                let mut failures = self.failure_count.write().await;
                *failures += 1;

                if *failures >= self.config.failure_threshold {
                    *state = CircuitState::Open;
                    *self.last_failure.write().await = std::time::Instant::now();
                    tracing::warn!(
                        "Circuit breaker opened after {} failures",
                        self.config.failure_threshold
                    );
                }
            }
            CircuitState::HalfOpen => {
                // Test call failed, re-open circuit
                *state = CircuitState::Open;
                *self.last_failure.write().await = std::time::Instant::now();
                tracing::warn!("Circuit breaker re-opened after test failure");
            }
            CircuitState::Open => {}
        }
    }

    /// Get current circuit state
    pub async fn current_state(&self) -> CircuitState {
        *self.state.read().await
    }

    /// Execute an operation with circuit breaker protection
    /// Returns Err(()) if circuit is open, otherwise passes through operation result
    pub async fn call<F, Fut, T, E>(&self, operation: F) -> Result<T, Option<E>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>>,
    {
        if !self.is_allowed().await {
            return Err(None);
        }

        let result = operation().await;

        match result {
            Ok(value) => {
                self.record_success().await;
                Ok(value)
            }
            Err(err) => {
                self.record_failure().await;
                Err(Some(err))
            }
        }
    }

    /// Get statistics about the circuit breaker
    pub async fn stats(&self) -> CircuitBreakerStats {
        let state = *self.state.read().await;
        let failures = *self.failure_count.read().await;
        let successes = *self.success_count.read().await;
        let last_failure_elapsed = self.last_failure.read().await.elapsed();

        CircuitBreakerStats {
            state,
            failures,
            successes,
            last_failure_elapsed,
            recovery_timeout: self.config.recovery_timeout,
        }
    }
}

/// Statistics about circuit breaker state for metrics/observability
#[derive(Debug, Clone)]
pub struct CircuitBreakerStats {
    pub state: CircuitState,
    pub failures: u32,
    pub successes: u32,
    pub last_failure_elapsed: Duration,
    pub recovery_timeout: Duration,
}

/// Convenience macro for common retry patterns
#[macro_export]
macro_rules! retryable {
    ($operation:expr, $name:expr) => {
        retry_with_backoff(
            RetryConfig::default(),
            || $operation,
            |_| true,
            $name,
        )
        .await
    };
    ($operation:expr, $name:expr, $config:expr) => {
        retry_with_backoff($config, || $operation, |_| true, $name).await
    };
}

/// Check if an error is potentially retryable
pub fn is_retryable_error(err: &reqwest::Error) -> bool {
    if err.is_connect() || err.is_timeout() {
        return true;
    }

    if let Some(status) = err.status() {
        return status.is_server_error()
            || status.as_u16() == 429 /* Too Many Requests */
            || status.as_u16() == 408 /* Request Timeout */;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retry_config_calculation() {
        let config = RetryConfig::default();

        // Delay should increase with each attempt
        let delay1 = config.calculate_delay(0);
        let delay2 = config.calculate_delay(1);
        let delay3 = config.calculate_delay(2);

        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
        assert!(delay3 <= config.max_delay);
    }

    #[tokio::test]
    async fn test_circuit_breaker_transitions() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
        };

        let cb = CircuitBreaker::new(config);

        // Initially closed and allowing calls
        assert_eq!(cb.current_state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await);

        // Record failures to trip circuit
        cb.record_failure().await;
        cb.record_failure().await;
        cb.record_failure().await;

        assert_eq!(cb.current_state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Should transition to half-open and allow test call
        assert!(cb.is_allowed().await);
        assert_eq!(cb.current_state().await, CircuitState::HalfOpen);

        // Successful recovery tests
        cb.record_success().await;
        cb.record_success().await;

        // Should be closed again
        assert_eq!(cb.current_state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_starts_closed() {
        let config = CircuitBreakerConfig::default();
        let cb = CircuitBreaker::new(config);

        assert_eq!(cb.current_state().await, CircuitState::Closed);
        assert!(cb.is_allowed().await);

        let stats = cb.stats().await;
        assert_eq!(stats.state, CircuitState::Closed);
        assert_eq!(stats.failures, 0);
        assert_eq!(stats.successes, 0);
    }

    #[tokio::test]
    async fn test_circuit_breaker_opens_after_failures() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            recovery_timeout: Duration::from_secs(30),
            success_threshold: 2,
        };

        let cb = CircuitBreaker::new(config);

        // Circuit should be closed initially
        assert_eq!(cb.current_state().await, CircuitState::Closed);

        // Record failures up to threshold - 1
        for _ in 0..4 {
            cb.record_failure().await;
            assert_eq!(cb.current_state().await, CircuitState::Closed);
            assert!(cb.is_allowed().await);
        }

        // One more failure should open the circuit
        cb.record_failure().await;
        assert_eq!(cb.current_state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);

        let stats = cb.stats().await;
        assert_eq!(stats.state, CircuitState::Open);
        assert_eq!(stats.failures, 5);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_after_timeout() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(100),
            success_threshold: 2,
        };

        let cb = CircuitBreaker::new(config);

        // Trip the circuit
        cb.record_failure().await;
        cb.record_failure().await;

        assert_eq!(cb.current_state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(150)).await;

        // First call to is_allowed should transition to HalfOpen
        assert!(cb.is_allowed().await);
        assert_eq!(cb.current_state().await, CircuitState::HalfOpen);

        let stats = cb.stats().await;
        assert_eq!(stats.state, CircuitState::HalfOpen);
        assert!(stats.last_failure_elapsed >= Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_circuit_breaker_resets_failures_on_success_when_closed() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            recovery_timeout: Duration::from_secs(1),
            success_threshold: 2,
        };

        let cb = CircuitBreaker::new(config);

        // Record some failures (but not enough to open)
        cb.record_failure().await;
        cb.record_failure().await;

        let stats = cb.stats().await;
        assert_eq!(stats.failures, 2);

        // Success should reset failure count
        cb.record_success().await;

        let stats = cb.stats().await;
        assert_eq!(stats.failures, 0);
        assert_eq!(cb.current_state().await, CircuitState::Closed);
    }

    #[tokio::test]
    async fn test_circuit_breaker_half_open_reopens_on_failure() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 2,
        };

        let cb = CircuitBreaker::new(config);

        // Open the circuit
        cb.record_failure().await;
        cb.record_failure().await;
        assert_eq!(cb.current_state().await, CircuitState::Open);

        // Wait for recovery
        tokio::time::sleep(Duration::from_millis(75)).await;

        // Transition to half-open
        assert!(cb.is_allowed().await);
        assert_eq!(cb.current_state().await, CircuitState::HalfOpen);

        // Failure in half-open should re-open circuit
        cb.record_failure().await;
        assert_eq!(cb.current_state().await, CircuitState::Open);
        assert!(!cb.is_allowed().await);
    }

    #[tokio::test]
    async fn test_circuit_breaker_call_method() {
        let config = CircuitBreakerConfig {
            failure_threshold: 2,
            recovery_timeout: Duration::from_millis(50),
            success_threshold: 1,
        };

        let cb = CircuitBreaker::new(config);

        // Successful call
        let result = cb.call(|| async { Ok::<_, String>(42) }).await;
        assert_eq!(result, Ok(42));

        // Failed calls
        let result = cb.call(|| async { Err::<i32, _>("error1".to_string()) }).await;
        assert_eq!(result, Err(Some("error1".to_string())));

        let result = cb.call(|| async { Err::<i32, _>("error2".to_string()) }).await;
        assert_eq!(result, Err(Some("error2".to_string())));

        // Circuit should be open now
        assert_eq!(cb.current_state().await, CircuitState::Open);

        // Call should be rejected (circuit open)
        let result = cb.call(|| async { Ok::<_, String>(100) }).await;
        assert_eq!(result, Err(None)); // None indicates circuit is open
    }

    #[tokio::test]
    async fn test_retry_with_backoff_succeeds_immediately() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        };

        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        let result = retry_with_backoff(
            config,
            move || {
                let a = attempts_clone.clone();
                async move {
                    a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Ok::<_, String>(42)
                }
            },
            |_| true,
            "test_operation",
        )
        .await;

        assert_eq!(result, Ok(42));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn test_retry_with_backoff_retries_on_failure() {
        let config = RetryConfig {
            max_retries: 3,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        };

        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        let result = retry_with_backoff(
            config,
            move || {
                let a = attempts_clone.clone();
                async move {
                    let count = a.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
                    if count < 3 {
                        Err("temporary error")
                    } else {
                        Ok(42)
                    }
                }
            },
            |_| true,
            "test_operation",
        )
        .await;

        assert_eq!(result, Ok(42));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 3);
    }

    #[tokio::test]
    async fn test_retry_with_backoff_respects_max_retries() {
        let config = RetryConfig {
            max_retries: 2,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        };

        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        let result = retry_with_backoff(
            config,
            move || {
                let a = attempts_clone.clone();
                async move {
                    a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err::<i32, _>("persistent error")
                }
            },
            |_| true,
            "test_operation",
        )
        .await;

        assert_eq!(result, Err("persistent error"));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_retry_with_backoff_respects_should_retry() {
        let config = RetryConfig {
            max_retries: 5,
            initial_delay: Duration::from_millis(10),
            max_delay: Duration::from_secs(1),
            backoff_multiplier: 2.0,
            jitter_factor: 0.1,
        };

        let attempts = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        let result = retry_with_backoff(
            config,
            move || {
                let a = attempts_clone.clone();
                async move {
                    a.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    Err::<i32, _>("non-retryable error")
                }
            },
            |_| false,
            "test_operation",
        )
        .await;

        assert_eq!(result, Err("non-retryable error"));
        assert_eq!(attempts.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn test_retry_config_presets() {
        let aggressive = RetryConfig::aggressive();
        assert_eq!(aggressive.max_retries, 5);
        assert!(aggressive.initial_delay < Duration::from_millis(100));

        let conservative = RetryConfig::conservative();
        assert_eq!(conservative.max_retries, 2);
        assert!(conservative.initial_delay >= Duration::from_millis(100));

        let default = RetryConfig::default();
        assert_eq!(default.max_retries, 3);
    }
}
