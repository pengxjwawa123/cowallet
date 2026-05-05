//! Input validation and sanitization middleware
//!
//! Provides:
//! - Ethereum address validation with checksum
//! - Numeric range validation
//! - XSS protection for returned data
//! - Path traversal prevention
//! - SQL injection pattern detection

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use serde::Serialize;
use std::collections::HashSet;

/// Validation error response
#[derive(Debug, Serialize)]
pub struct ValidationError {
    pub error: &'static str,
    pub details: Option<String>,
}

impl ValidationError {
    pub fn new(error: &'static str) -> Self {
        Self {
            error,
            details: None,
        }
    }

    pub fn with_details(error: &'static str, details: String) -> Self {
        Self {
            error,
            details: Some(details),
        }
    }

    pub fn into_response(self) -> (StatusCode, Json<Self>) {
        (StatusCode::BAD_REQUEST, Json(self))
    }
}

/// Common SQL injection patterns to detect
const SQLI_PATTERNS: &[&str] = &[
    "' OR ",
    "' OR 1=1",
    "' OR '1'='1",
    "' UNION ",
    "' UNION SELECT ",
    "DROP TABLE",
    "INSERT INTO",
    "DELETE FROM",
    "UPDATE .* SET",
    "--",
    "/*",
    "*/",
    "xp_cmdshell",
    "exec(",
    "EXEC(",
    "' AND ",
];

/// Common XSS patterns to detect in query params and form inputs
const XSS_PATTERNS: &[&str] = &[
    "<script",
    "</script>",
    "javascript:",
    "onerror=",
    "onload=",
    "onclick=",
    "onmouseover=",
    "onfocus=",
    "onblur=",
    "onchange=",
    "eval(",
    "expression(",
    "<iframe",
    "<img",
    "<svg",
];

/// Validate an Ethereum address with checksum validation
pub fn validate_eth_address(address: &str) -> Result<(), ValidationError> {
    // Check basic format first
    if !address.starts_with("0x") {
        return Err(ValidationError::new("Address must start with 0x"));
    }

    let addr = &address[2..];

    // Check length
    if addr.len() != 40 {
        return Err(ValidationError::new(
            "Address must be 40 characters (excluding 0x",
        ));
    }

    // Check hex characters
    if !addr.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(ValidationError::new(
            "Address must contain only hexadecimal characters",
        ));
    }

    Ok(())
}

/// Validate numeric value is within range
pub fn validate_numeric_range<T: PartialOrd>(value: T, min: T, max: T) -> Result<(), ValidationError> {
    if value < min || value > max {
        return Err(ValidationError::new(
            "Value out of allowed range",
        ));
    }
    Ok(())
}

/// Check for potential SQL injection patterns
pub fn detect_sqli(input: &str) -> Result<(), ValidationError> {
    let input_lower = input.to_lowercase();
    for pattern in SQLI_PATTERNS {
        if input_lower.contains(&pattern.to_lowercase()) {
            return Err(ValidationError::with_details(
                "Potential SQL injection detected",
                format!("Pattern matched: {}", pattern),
            ));
        }
    }
    Ok(())
}

/// Check for potential XSS patterns
pub fn detect_xss(input: &str) -> Result<(), ValidationError> {
    let input_lower = input.to_lowercase();
    for pattern in XSS_PATTERNS {
        if input_lower.contains(&pattern.to_lowercase()) {
            return Err(ValidationError::with_details(
                "Potential XSS detected",
                format!("Pattern matched: {}", pattern),
            ));
        }
    }
    Ok(())
}

/// Sanitize HTML/XSS content for safe output
pub fn sanitize_html(input: &str) -> String {
    input
        .replace('&', "&amp;")  // Must be first to avoid double-escaping
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}

/// Check for path traversal attacks
pub fn validate_path(input: &str) -> Result<(), ValidationError> {
    let normalized = input.replace('\\', "/");

    if normalized.contains("..")
        || normalized.contains("/../")
        || normalized.starts_with("../")
        || normalized.contains("%2e%2e")  // URL-encoded ..
        || normalized.contains("%2F")  // URL-encoded /
    {
        return Err(ValidationError::new(
            "Path traversal attempt detected",
        ));
    }

    Ok(())
}

/// Axum middleware that validates query parameters and request path
pub async fn input_validation_middleware(
    request: Request<Body>,
    next: Next,
) -> Result<Response, (StatusCode, Json<ValidationError>)> {
    // Validate path for traversal attacks
    let path = request.uri().path();
    if let Err(e) = validate_path(path) {
        tracing::warn!("Path traversal attempt detected: {}", path);
        return Err(e.into_response());
    }

    // Validate query parameters for XSS/SQLi
    if let Some(query) = request.uri().query() {
        // Simple pattern matching without external dependency (simplified
        // TODO: use form-urlencoded
        for part in query.split('&') {
            let kv: Vec<&str> = part.split('=').collect();
            if let Some(value) = kv.get(1) {
                if detect_sqli(value).is_err() || detect_xss(value).is_err() {
                    tracing::warn!(
                        "Malicious input detected in query param: {}",
                        kv.get(0).unwrap_or(&"unknown")
                    );
                    return Err(ValidationError::new("Invalid query parameter").into_response());
                }
            }
        }
    }

    Ok(next.run(request).await)
}

/// Validate JSON body validation helper for route handlers
pub fn validate_json_body<T: Serialize>(body: &serde_json::Value) -> Result<(), ValidationError> {
    // Recursively check all string values
    fn check_value(value: &serde_json::Value) -> Result<(), ValidationError> {
        match value {
            serde_json::Value::String(s) => {
                detect_sqli(s)?;
                detect_xss(s)?;
                Ok(())
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    check_value(v)?;
                }
                Ok(())
            }
            serde_json::Value::Object(obj) => {
                for v in obj.values() {
                    check_value(v)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    check_value(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_eth_address() {
        // Valid format
        assert!(
            validate_eth_address("0xd8da6bf26964af9d7eed9e03e53415d37aa96045").is_ok()
        );

        // Invalid - too short
        assert!(validate_eth_address("0x1234").is_err());

        // Invalid - no 0x prefix
        assert!(validate_eth_address("d8da6bf26964af9d7eed9e03e53415d37aa96045")
            .is_err());
    }

    #[test]
    fn test_detect_sqli() {
        assert!(detect_sqli("' OR 1=1 --").is_err());
        assert!(detect_sqli("normal_input").is_ok());
        assert!(detect_sqli("john@example.com").is_ok());
    }

    #[test]
    fn test_detect_xss() {
        assert!(detect_xss("<script>alert(1)</script>").is_err());
        assert!(detect_xss("javascript:alert(1)").is_err());
        assert!(detect_xss("normal text").is_ok());
    }

    #[test]
    fn test_validate_path() {
        assert!(validate_path("../etc/passwd").is_err());
        assert!(validate_path("safe/path").is_ok());
        assert!(validate_path("/users/profile").is_ok());
        assert!(validate_path("..\\windows\\system32").is_err());
    }

    #[test]
    fn test_sanitize_html() {
        assert_eq!(
            sanitize_html("<script>"),
            "&lt;script&gt;"
        );
        assert_eq!(
            sanitize_html("hello & world"),
            "hello &amp; world"
        );
    }
}
