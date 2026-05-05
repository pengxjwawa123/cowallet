mod errors;
mod middleware;
mod retry;
mod routes;
mod services;
mod state;

use axum::{Router, extract::{State, Extension}, middleware as axum_mw, response::Json, routing::get};
use axum::http::{Method, header, StatusCode, Request};
use axum::body::Body;
use serde_json::json;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::compression::CompressionLayer;
use tower_http::timeout::TimeoutLayer;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::trace::{TraceLayer, MakeSpan, OnResponse};
use tracing_subscriber;
use std::time::Duration;

use middleware::audit::audit_middleware;
use middleware::auth::require_auth;
use middleware::metrics::metrics_middleware;
use middleware::rate_limit::{strict_rate_limit_middleware, standard_rate_limit_middleware, auth_rate_limit_middleware};
use middleware::request_id::request_id_middleware;
use middleware::security_headers::SecurityHeadersLayer;
use middleware::validation::input_validation_middleware;
use state::AppState;

fn cors_layer() -> CorsLayer {
    let methods = [Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS];
    let headers = [header::AUTHORIZATION, header::CONTENT_TYPE, header::ACCEPT];

    let allowed = std::env::var("CORS_ALLOWED_ORIGINS").unwrap_or_default();
    if allowed.is_empty() || allowed == "*" {
        tracing::warn!("CORS_ALLOWED_ORIGINS not set — defaulting to localhost only");
        CorsLayer::new()
            .allow_origin(AllowOrigin::list([
                "http://localhost:3000".parse().unwrap(),
                "http://localhost:8080".parse().unwrap(),
            ]))
            .allow_methods(methods)
            .allow_headers(headers)
    } else {
        let origins: Vec<_> = allowed
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_methods(methods)
            .allow_headers(headers)
    }
}

/// Panic hook that logs the panic before aborting
fn setup_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::capture();
        tracing::error!(
            "PANIC occurred.\nInfo: {}\nBacktrace:\n{}",
            info,
            backtrace
        );
        // Call the default hook after logging
        default_hook(info);
    }));
}

#[tokio::main]
async fn main() {
    // Initialize tracing and panic handling first
    tracing_subscriber::fmt::init();
    setup_panic_hook();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://localhost/cowallet".into());
    let rpc_url =
        std::env::var("RPC_URL").unwrap_or_else(|_| "https://sepolia.base.org".into());

    let app_state = match AppState::new(&database_url, rpc_url).await {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("DB unavailable ({e}), starting without database");
            AppState::without_db()
        }
    };

    // MPC routes with strict rate limiting (10 req/min)
    let mpc_routes = Router::new()
        .nest("/mpc", routes::mpc::router())
        .layer(axum_mw::from_fn(strict_rate_limit_middleware));

    // Initialize encryption service (in production, key from KMS/HSM)
    let encryption_key = std::env::var("ENCRYPTION_KEY")
        .ok()
        .and_then(|k| hex::decode(&k).ok())
        .unwrap_or_else(|| {
            tracing::warn!("Using default encryption key - NOT FOR PRODUCTION!");
            (0..32).collect()
        });

    let mut key_array = [0u8; 32];
    if encryption_key.len() == 32 {
        key_array.copy_from_slice(&encryption_key);
    }

    let encryption = services::crypto::EncryptionService::new(
        &key_array,
        "default-key",
    );

    // Protected routes with standard rate limiting (100 req/min)
    let protected = Router::new()
        .merge(mpc_routes)
        .nest("/tx", routes::tx::router())
        .nest("/policy", routes::policy::router())
        .nest("/ai", routes::ai::router())
        .nest("/yield", routes::yield_::router())
        .nest("/shards", routes::shards::router())
        .layer(Extension(encryption))
        .layer(axum_mw::from_fn(require_auth))
        .layer(axum_mw::from_fn(standard_rate_limit_middleware));

    // Clone app_state for shutdown handler
    let app_state_clone = app_state.clone();

    // Tracing layer with request IDs for observability
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &Request<Body>| {
            let request_id = request
                .headers()
                .get("x-request-id")
                .and_then(|h| h.to_str().ok())
                .unwrap_or("unknown");

            tracing::info_span!(
                "request",
                method = %request.method(),
                uri = %request.uri(),
                version = ?request.version(),
                request_id = %request_id,
            )
        })
        .on_response(|response: &axum::http::Response<_>, latency: Duration, _span: &tracing::Span| {
            tracing::info!(
                status = %response.status(),
                latency = %format_args!("{:?}", latency),
                "response sent"
            );
        });

    // Metrics endpoint handler - renders metrics from AppState
    async fn metrics(State(state): State<AppState>) -> impl axum::response::IntoResponse {
        use middleware::metrics::MetricsStore;

        let mut output = state.metrics.render();

        // Add circuit breaker stats
        let rpc_stats = state.rpc_circuit_breaker.stats().await;
        output.push_str(&MetricsStore::render_circuit_breaker_stats("rpc", &rpc_stats));

        let defi_stats = state.defi_circuit_breaker.stats().await;
        output.push_str(&MetricsStore::render_circuit_breaker_stats("defi", &defi_stats));

        // Add database connection pool stats
        if let Some(db) = &state.db {
            output.push_str(&MetricsStore::render_db_pool_stats(
                db.options().get_max_connections() as i64,
                0,  // Size and idle info not directly available in sqlx public API
                0,
            ));
        }

        (
            [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
            output,
        )
    }

    let app_state_for_middleware = app_state.clone();

    let app = Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/live", get(live))
        .route("/metrics", get(metrics))
        .nest("/api/v1/auth", routes::auth::router()
            .layer(axum_mw::from_fn(auth_rate_limit_middleware)))
        .nest("/api/v1/price", routes::price::router())
        .nest("/api/v1", protected)
        .with_state(app_state.clone())
        // Order: Outermost first (applied first to request, last to response)
        .layer(axum_mw::from_fn_with_state(app_state_for_middleware, |state: State<AppState>, mut request: Request<Body>, next: Next| async move {
            request.extensions_mut().insert(state.0);
            next.run(request).await
        }))
        .layer(SecurityHeadersLayer::new())
        .layer(CompressionLayer::new()) // gzip/brotli compression
        .layer(TimeoutLayer::new(Duration::from_secs(30))) // 30s request timeout
        .layer(RequestBodyLimitLayer::new(10 * 1024 * 1024)) // 10MB max body size
        .layer(trace_layer) // Structured logging
        .layer(axum_mw::from_fn(request_id_middleware))
        .layer(axum_mw::from_fn(input_validation_middleware))
        .layer(axum_mw::from_fn(audit_middleware))
        .layer(cors_layer());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("cowallet API server v{} listening on :3000", env!("CARGO_PKG_VERSION"));

    // Graceful shutdown setup
    let server = axum::serve(listener, app);

    let graceful = server.with_graceful_shutdown(async {
        let mut sigint = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())
            .expect("failed to install SIGINT handler");
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler");

        tokio::select! {
            _ = sigint.recv() => tracing::info!("Received SIGINT, initiating graceful shutdown"),
            _ = sigterm.recv() => tracing::info!("Received SIGTERM, initiating graceful shutdown"),
        }
    });

    if let Err(e) = graceful.await {
        tracing::error!("server error: {}", e);
    }

    // Cleanup phase - wait for in-flight requests and clean up resources
    tracing::info!("Waiting for in-flight requests to complete...");
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    // Gracefully close database connections
    if let Some(db) = app_state_clone.db {
        tracing::info!("Closing database connections...");
        db.close().await;
        tracing::info!("Database connections closed");
    }

    tracing::info!("Server shutdown complete");
}

/// Production-grade health check endpoint
#[derive(serde::Serialize)]
struct HealthResponse {
    status: &'static str,
    timestamp: String,
    version: &'static str,
    uptime_seconds: u64,
    services: ServiceHealth,
}

#[derive(serde::Serialize)]
struct ServiceHealth {
    database: &'static str,
    rpc: &'static str,
    yield_cache: &'static str,
}

/// Global service start time for uptime calculation
static START_TIME: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();

async fn health(State(state): State<AppState>) -> (StatusCode, Json<HealthResponse>) {
    // Calculate uptime
    let uptime = START_TIME
        .get_or_init(std::time::Instant::now)
        .elapsed()
        .as_secs();

    // Database health check with actual connection test
    let db_status = match &state.db {
        Some(pool) => {
            // Try a simple query to verify connection is alive
            match sqlx::query("SELECT 1").fetch_one(pool).await {
                Ok(_) => "healthy",
                Err(e) => {
                    tracing::warn!("Database health check failed: {}", e);
                    "unhealthy"
                }
            }
        }
        None => "degraded",
    };

    // RPC health check (simple URL validation for now)
    let rpc_status = if state.rpc_url.contains("http") { "healthy" } else { "degraded" };

    // Yield cache status
    let cache_data = state.yield_cache.data.read().await;
    let cache_status = if cache_data.is_empty() {
        "warming"
    } else {
        "healthy"
    };

    // Determine overall status
    let overall_status = if db_status == "healthy" && rpc_status == "healthy" {
        "healthy"
    } else if db_status == "unhealthy" {
        "unhealthy"
    } else {
        "degraded"
    };

    let status_code = match overall_status {
        "healthy" => StatusCode::OK,
        "degraded" => StatusCode::OK, // Still accept traffic in degraded mode
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };

    (status_code, Json(HealthResponse {
        status: overall_status,
        timestamp: chrono::Utc::now().to_rfc3339(),
        version: env!("CARGO_PKG_VERSION"),
        uptime_seconds: uptime,
        services: ServiceHealth {
            database: db_status,
            rpc: rpc_status,
            yield_cache: cache_status,
        },
    }))
}

/// Ready probe for Kubernetes - indicates service can accept traffic
async fn ready(State(state): State<AppState>) -> (StatusCode, Json<serde_json::Value>) {
    // Check if we have essential services available
    let db_ready = state.db.is_some();

    if db_ready {
        (StatusCode::OK, Json(json!({ "ready": true })))
    } else {
        (StatusCode::SERVICE_UNAVAILABLE, Json(json!({ "ready": false })))
    }
}

/// Live probe for Kubernetes - indicates service is still running
async fn live() -> StatusCode {
    StatusCode::OK
}
