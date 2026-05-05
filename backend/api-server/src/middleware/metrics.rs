//! Simple request timing metrics middleware.
//!
//! Tracks request counts, latencies, and provides a metrics endpoint.
//! Uses a simple in-memory store for basic observability.

use axum::{
    body::Body,
    extract::State,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Global metrics store
#[derive(Debug, Clone, Default)]
pub struct MetricsStore {
    inner: Arc<Mutex<MetricsData>>,
}

#[derive(Debug)]
struct MetricsData {
    request_count: HashMap<String, u64>, // (method_path_status) -> count
    error_count: u64,
    total_requests: u64,
    start_time: Instant,
}

impl Default for MetricsData {
    fn default() -> Self {
        Self {
            request_count: HashMap::new(),
            error_count: 0,
            total_requests: 0,
            start_time: Instant::now(),
        }
    }
}

impl MetricsStore {
    /// Create a new metrics store
    pub fn new() -> Self {
        let store = Self {
            inner: Arc::new(Mutex::new(MetricsData {
                request_count: HashMap::new(),
                error_count: 0,
                total_requests: 0,
                start_time: Instant::now(),
            })),
        };
        tracing::info!("Metrics store initialized");
        store
    }

    /// Record a request with its status
    pub fn record_request(&self, method: &str, path: &str, status: StatusCode, duration_ms: u64) {
        let mut inner = self.inner.lock().unwrap();
        inner.total_requests += 1;

        let key = format!("{}:{}:{}", method, path, status.as_str());
        *inner.request_count.entry(key).or_insert(0) += 1;

        if status.is_server_error() {
            inner.error_count += 1;
        }
    }

    /// Get current metrics as a string (Prometheus format)
    pub fn render(&self) -> String {
        let inner = self.inner.lock().unwrap();
        let uptime = inner.start_time.elapsed().as_secs();

        let mut output = String::new();

        output.push_str(&format!(
            "# HELP http_requests_total Total number of HTTP requests\n"
        ));
        output.push_str(&format!(
            "# TYPE http_requests_total counter\n"
        ));
        output.push_str(&format!(
            "http_requests_total {}\n",
            inner.total_requests
        ));

        output.push_str(&format!(
            "# HELP http_request_errors_total Total number of HTTP 5xx errors\n"
        ));
        output.push_str(&format!(
            "# TYPE http_request_errors_total counter\n"
        ));
        output.push_str(&format!(
            "http_request_errors_total {}\n",
            inner.error_count
        ));

        output.push_str(&format!(
            "# HELP process_uptime_seconds Service uptime in seconds\n"
        ));
        output.push_str(&format!(
            "# TYPE process_uptime_seconds gauge\n"
        ));
        output.push_str(&format!(
            "process_uptime_seconds {}\n",
            uptime
        ));

        // Per-route stats
        for (key, count) in &inner.request_count {
            let parts: Vec<&str> = key.split(':').collect();
            if parts.len() == 3 {
                output.push_str(&format!(
                    "http_requests_by_route{{method=\"{}\",path=\"{}\",status=\"{}\"}} {}\n",
                    parts[0], parts[1], parts[2], count
                ));
            }
        }

        output
    }

    /// Render circuit breaker statistics into Prometheus format
    pub fn render_circuit_breaker_stats(name: &str, stats: &crate::retry::CircuitBreakerStats) -> String {
        let state_str = match stats.state {
            crate::retry::CircuitState::Closed => "closed",
            crate::retry::CircuitState::Open => "open",
            crate::retry::CircuitState::HalfOpen => "half_open",
        };

        format!(
            "# HELP circuit_breaker_state Current state of the circuit breaker (0=closed,1=open,2=half_open)\n\
             # TYPE circuit_breaker_state gauge\n\
             circuit_breaker_state{{service=\"{name}\",state=\"{state_str}\"}} {}\n\
             # HELP circuit_breaker_failures Failure count for circuit breaker\n\
             # TYPE circuit_breaker_failures counter\n\
             circuit_breaker_failures{{service=\"{name}\"}} {}\n\
             # HELP circuit_breaker_recovery_seconds Seconds until circuit breaker attempts recovery\n\
             # TYPE circuit_breaker_recovery_seconds gauge\n\
             circuit_breaker_recovery_seconds{{service=\"{name}\"}} {}\n",
            stats.state as i32,
            stats.failures,
            if state_str == "open" {
                stats.recovery_timeout.as_secs().saturating_sub(stats.last_failure_elapsed.as_secs())
            } else {
                0
            }
        )
    }

    /// Render database connection pool statistics into Prometheus format
    pub fn render_db_pool_stats(
        max_connections: i64,
        num_connections: i64,
        num_idle: i64,
    ) -> String {
        format!(
            "# HELP db_pool_max_connections Maximum database connections in pool\n\
             # TYPE db_pool_max_connections gauge\n\
             db_pool_max_connections {}\n\
             # HELP db_pool_active_connections Active database connections in pool\n\
             # TYPE db_pool_active_connections gauge\n\
             db_pool_active_connections {}\n\
             # HELP db_pool_idle_connections Idle database connections in pool\n\
             # TYPE db_pool_idle_connections gauge\n\
             db_pool_idle_connections {}\n",
            max_connections,
            num_connections.saturating_sub(num_idle),
            num_idle
        )
    }
}

/// Initialize the metrics store and return a clone
pub fn init_metrics() -> MetricsStore {
    MetricsStore::new()
}

/// Metrics middleware that tracks request counts and latencies
pub async fn metrics_middleware(
    State(metrics): State<MetricsStore>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = request.uri().path().to_string();

    let response = next.run(request).await;

    let duration = start.elapsed();
    let status = response.status();

    metrics.record_request(&method, &path, status, duration.as_millis() as u64);

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_metrics_store() {
        let store = MetricsStore::new();
        store.record_request("GET", "/health", StatusCode::OK, 5);
        store.record_request("POST", "/api/v1/auth", StatusCode::OK, 20);

        let output = store.render();
        assert!(output.contains("http_requests_total 2"));
        assert!(output.contains("process_uptime_seconds"));
    }
}
