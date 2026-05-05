//! Request ID middleware for tracing and log correlation
//!
//! Generates unique request IDs and injects them into:
//! - Response headers (X-Request-ID)
//! - Tracing spans
//! - Request extensions for access in handlers

use axum::{
    body::Body,
    http::{HeaderName, Request, header},
    middleware::Next,
    response::Response,
};
use std::sync::atomic::{AtomicU64, Ordering};
use uuid::Uuid;

/// Request ID stored in request extensions
#[derive(Debug, Clone)]
pub struct RequestId(pub String);

/// Atomic counter for request sequence numbers (useful for high-throughput logging)
static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Generate a new request ID
/// Format: UUID v4 (or falls back to counter-based if UUID generation fails)
pub fn generate_request_id() -> String {
    Uuid::new_v4().to_string()
}

/// Get the next sequence number (for additional correlation)
#[allow(dead_code)]
pub fn next_sequence() -> u64 {
    REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Middleware that injects request IDs into the request/response cycle
pub async fn request_id_middleware(mut request: Request<Body>, next: Next) -> Response {
    // Try to get request ID from incoming header (for cross-service propagation)
    let request_id = request
        .headers()
        .get("X-Request-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(generate_request_id);

    // Store in request extensions for handler access
    request.extensions_mut().insert(RequestId(request_id.clone()));

    // Create a tracing span with the request ID
    let span = tracing::info_span!("request", request_id = %request_id);
    let _enter = span.enter();

    tracing::debug!("Processing request");

    // Process request
    let mut response = next.run(request).await;

    // Inject request ID into response headers
    response.headers_mut().insert(
        HeaderName::from_static("x-request-id"),
        header::HeaderValue::from_str(&request_id).unwrap_or(header::HeaderValue::from_static("unknown")),
    );

    response
}
