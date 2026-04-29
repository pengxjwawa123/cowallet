mod middleware;
mod routes;
mod state;

use axum::{Router, middleware as axum_mw, routing::get};
use axum::http::{Method, header};
use tower_http::cors::{AllowOrigin, CorsLayer};
use tracing_subscriber;

use middleware::auth::require_auth;
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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

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

    let protected = Router::new()
        .nest("/mpc", routes::mpc::router())
        .nest("/tx", routes::tx::router())
        .nest("/policy", routes::policy::router())
        .nest("/ai", routes::ai::router())
        .layer(axum_mw::from_fn(require_auth));

    let app = Router::new()
        .route("/health", get(health))
        .nest("/api/v1/auth", routes::auth::router())
        .nest("/api/v1/price", routes::price::router())
        .nest("/api/v1", protected)
        .with_state(app_state)
        .layer(cors_layer());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("cowallet API server listening on :3000");
    axum::serve(listener, app).await.unwrap();
}

async fn health() -> &'static str {
    "ok"
}
