//! Production-grade security headers middleware.
//!
//! Implements OWASP recommended security headers:
//! - HSTS (HTTP Strict Transport Security)
//! - CSP (Content Security Policy)
//! - X-Frame-Options
//! - X-Content-Type-Options
//! - Referrer-Policy
//! - Permissions-Policy
//! - X-XSS-Protection

use axum::{
    http::header::{HeaderName, HeaderValue},
    response::Response,
};
use futures::future::BoxFuture;
use std::task::{Context, Poll};
use tower::{Layer, Service};

/// Layer that applies security headers to all responses.
#[derive(Debug, Clone)]
pub struct SecurityHeadersLayer {
    hsts_max_age: u64,
    hsts_include_subdomains: bool,
    enable_csp: bool,
}

impl Default for SecurityHeadersLayer {
    fn default() -> Self {
        Self {
            hsts_max_age: 31536000, // 1 year
            hsts_include_subdomains: true,
            enable_csp: true,
        }
    }
}

impl SecurityHeadersLayer {
    /// Create a new security headers layer with production defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Customize HSTS max age (in seconds)
    pub fn hsts_max_age(mut self, max_age: u64) -> Self {
        self.hsts_max_age = max_age;
        self
    }

    /// Disable HSTS includeSubDomains
    pub fn disable_hsts_subdomains(mut self) -> Self {
        self.hsts_include_subdomains = false;
        self
    }

    /// Disable CSP (not recommended for production)
    pub fn disable_csp(mut self) -> Self {
        self.enable_csp = false;
        self
    }
}

impl<S> Layer<S> for SecurityHeadersLayer {
    type Service = SecurityHeadersService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SecurityHeadersService {
            inner,
            config: self.clone(),
        }
    }
}

/// Service that applies security headers to responses.
#[derive(Debug, Clone)]
pub struct SecurityHeadersService<S> {
    inner: S,
    config: SecurityHeadersLayer,
}

impl<S, ReqBody, ResBody> Service<axum::http::Request<ReqBody>> for SecurityHeadersService<S>
where
    S: Service<axum::http::Request<ReqBody>, Response = Response<ResBody>>,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: axum::http::Request<ReqBody>) -> Self::Future {
        let config = self.config.clone();
        let future = self.inner.call(req);

        Box::pin(async move {
            let mut response = future.await?;
            let headers = response.headers_mut();

            // HSTS - HTTP Strict Transport Security
            let hsts_value = if config.hsts_include_subdomains {
                format!("max-age={}; includeSubDomains", config.hsts_max_age)
            } else {
                format!("max-age={}", config.hsts_max_age)
            };
            headers.insert(
                HeaderName::from_static("strict-transport-security"),
                HeaderValue::from_str(&hsts_value).unwrap(),
            );

            // X-Frame-Options - Prevent clickjacking
            headers.insert(
                HeaderName::from_static("x-frame-options"),
                HeaderValue::from_static("DENY"),
            );

            // X-Content-Type-Options - Prevent MIME sniffing
            headers.insert(
                HeaderName::from_static("x-content-type-options"),
                HeaderValue::from_static("nosniff"),
            );

            // Referrer-Policy - Control referrer information
            headers.insert(
                HeaderName::from_static("referrer-policy"),
                HeaderValue::from_static("strict-origin-when-cross-origin"),
            );

            // Permissions-Policy - Disable unused browser features
            headers.insert(
                HeaderName::from_static("permissions-policy"),
                HeaderValue::from_static(
                    "camera=(), microphone=(), geolocation=(), payment=(), usb=()"
                ),
            );

            // X-XSS-Protection - Disable legacy XSS auditor (modern approach)
            // Setting to 0 is recommended as the feature can introduce security vulnerabilities
            headers.insert(
                HeaderName::from_static("x-xss-protection"),
                HeaderValue::from_static("0"),
            );

            // CSP - Content Security Policy (API focused, minimal)
            // For API servers we use a restrictive policy since we don't serve HTML
            if config.enable_csp {
                headers.insert(
                    HeaderName::from_static("content-security-policy"),
                    HeaderValue::from_static(
                        "default-src 'none'; frame-ancestors 'none'; base-uri 'none'; form-action 'none'"
                    ),
                );
            }

            Ok(response)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request, Router, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_security_headers_applied() {
        let app = Router::new()
            .route("/test", get(|| async { "ok" }))
            .layer(SecurityHeadersLayer::new());

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        let headers = response.headers();

        assert!(headers.contains_key("strict-transport-security"));
        assert!(headers.contains_key("x-frame-options"));
        assert!(headers.contains_key("x-content-type-options"));
        assert!(headers.contains_key("referrer-policy"));
        assert!(headers.contains_key("permissions-policy"));
        assert!(headers.contains_key("x-xss-protection"));
        assert!(headers.contains_key("content-security-policy"));

        assert_eq!(
            headers.get("x-frame-options").unwrap(),
            "DENY"
        );
        assert_eq!(
            headers.get("x-content-type-options").unwrap(),
            "nosniff"
        );
    }
}
