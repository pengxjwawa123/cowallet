//! Production-grade error handling for CoWallet API
//!
//! Provides:
//! - Unified error type for all API errors
//! - Structured JSON error responses
//! - Error code enumeration for client-side handling
//! - Tracing integration for observability

use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
use std::fmt;

/// Standardized error codes for API responses
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    // Authentication errors (1000-1099)
    AuthMissingToken = 1000,
    AuthInvalidToken = 1001,
    AuthExpiredToken = 1002,
    AuthInsufficientScope = 1003,
    AuthForbidden = 1004,

    // Validation errors (1100-1199)
    ValidationFailed = 1100,
    InvalidAddress = 1101,
    InvalidAmount = 1102,
    MissingParameter = 1103,

    // Rate limiting (1200-1299)
    RateLimitExceeded = 1200,
    TooManyRequests = 1201,

    // Resource errors (1300-1399)
    ResourceNotFound = 1300,
    ResourceConflict = 1301,

    // MPC Protocol errors (1400-1499)
    MpcSessionNotFound = 1400,
    MpcInvalidMessage = 1401,
    MpcProtocolError = 1402,
    MpcSignatureFailed = 1403,

    // External service errors (1500-1599)
    RpcError = 1500,
    ExternalApiFailed = 1501,

    // Database errors (1600-1699)
    DatabaseError = 1600,
    DatabaseConflict = 1601,

    // Policy errors (1700-1799)
    PolicyViolation = 1700,
    TransactionBlocked = 1701,

    // Generic errors (1800-1899)
    InternalError = 1800,
    ServiceUnavailable = 1801,
    NotImplemented = 1802,
}

/// Standardized API error response structure
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    /// Machine-readable error code
    pub code: ErrorCode,
    /// Numeric error code (for backwards compatibility)
    pub status: u16,
    /// Human-readable error message
    pub message: String,
    /// Optional detailed error information (dev only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Error timestamp
    pub timestamp: String,
    /// Request ID for tracing correlation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

/// Primary error type for the CoWallet API
#[derive(Debug)]
pub struct ApiError {
    code: ErrorCode,
    status: StatusCode,
    message: String,
    details: Option<serde_json::Value>,
    source: Option<Box<dyn std::error::Error + Send + Sync>>,
    request_id: Option<String>,
}

impl ApiError {
    // --- Authentication errors ---

    pub fn auth_missing_token() -> Self {
        Self {
            code: ErrorCode::AuthMissingToken,
            status: StatusCode::UNAUTHORIZED,
            message: "Authentication token is required".into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    pub fn auth_invalid_token(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::AuthInvalidToken,
            status: StatusCode::UNAUTHORIZED,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    pub fn auth_forbidden(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::AuthForbidden,
            status: StatusCode::FORBIDDEN,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    // --- Validation errors ---

    pub fn validation_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ValidationFailed,
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    pub fn invalid_address(address: &str) -> Self {
        Self {
            code: ErrorCode::InvalidAddress,
            status: StatusCode::BAD_REQUEST,
            message: format!("Invalid address format: {}", address),
            details: None,
            source: None,
            request_id: None,
        }
    }

    pub fn missing_param(param: &str) -> Self {
        Self {
            code: ErrorCode::MissingParameter,
            status: StatusCode::BAD_REQUEST,
            message: format!("Missing required parameter: {}", param),
            details: None,
            source: None,
            request_id: None,
        }
    }

    // --- Rate limiting ---

    pub fn rate_limited(retry_after: u64) -> Self {
        Self {
            code: ErrorCode::RateLimitExceeded,
            status: StatusCode::TOO_MANY_REQUESTS,
            message: format!("Rate limit exceeded. Retry after {} seconds", retry_after),
            details: Some(serde_json::json!({ "retry_after": retry_after })),
            source: None,
            request_id: None,
        }
    }

    // --- Resource errors ---

    pub fn not_found(resource: &str, id: &str) -> Self {
        Self {
            code: ErrorCode::ResourceNotFound,
            status: StatusCode::NOT_FOUND,
            message: format!("{} not found: {}", resource, id),
            details: None,
            source: None,
            request_id: None,
        }
    }

    // --- MPC errors ---

    pub fn mpc_session_not_found(session_id: &str) -> Self {
        Self {
            code: ErrorCode::MpcSessionNotFound,
            status: StatusCode::NOT_FOUND,
            message: format!("MPC session not found: {}", session_id),
            details: None,
            source: None,
            request_id: None,
        }
    }

    pub fn mpc_protocol_error(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::MpcProtocolError,
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    // --- External service errors ---

    pub fn rpc_error(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::RpcError,
            status: StatusCode::BAD_GATEWAY,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    // --- Database errors ---

    pub fn database_error(err: sqlx::Error) -> Self {
        tracing::error!("Database error: {:?}", err);
        Self {
            code: ErrorCode::DatabaseError,
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: "A database error occurred".into(),
            details: None,
            source: Some(Box::new(err)),
            request_id: None,
        }
    }

    // --- Generic errors ---

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InternalError,
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    pub fn service_unavailable(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::ServiceUnavailable,
            status: StatusCode::SERVICE_UNAVAILABLE,
            message: message.into(),
            details: None,
            source: None,
            request_id: None,
        }
    }

    // --- Builder methods ---

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn with_source<E: std::error::Error + Send + Sync + 'static>(mut self, source: E) -> Self {
        self.source = Some(Box::new(source));
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ApiError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.as_ref().map(|e| e.as_ref() as &(dyn std::error::Error + 'static))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        // Log error with appropriate level based on status code
        if self.status.is_server_error() {
            tracing::error!(
                error_code = ?self.code,
                status = %self.status,
                message = %self.message,
                request_id = ?self.request_id,
                "Server error"
            );
        } else {
            tracing::warn!(
                error_code = ?self.code,
                status = %self.status,
                message = %self.message,
                request_id = ?self.request_id,
                "Client error"
            );
        }

        let error_response = ErrorResponse {
            code: self.code,
            status: self.status.as_u16(),
            message: self.message,
            details: self.details,
            timestamp: chrono::Utc::now().to_rfc3339(),
            request_id: self.request_id,
        };

        (self.status, Json(error_response)).into_response()
    }
}

// Convenience type alias for results
pub type Result<T, E = ApiError> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_status_codes() {
        // Authentication errors - 401 Unauthorized
        assert_eq!(ApiError::auth_missing_token().status, StatusCode::UNAUTHORIZED);
        assert_eq!(ApiError::auth_invalid_token("test").status, StatusCode::UNAUTHORIZED);

        // Forbidden - 403
        assert_eq!(ApiError::auth_forbidden("test").status, StatusCode::FORBIDDEN);

        // Validation errors - 400 Bad Request
        assert_eq!(ApiError::validation_failed("test").status, StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::invalid_address("0x123").status, StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::missing_param("user_id").status, StatusCode::BAD_REQUEST);
        assert_eq!(ApiError::mpc_protocol_error("test").status, StatusCode::BAD_REQUEST);

        // Rate limiting - 429 Too Many Requests
        assert_eq!(ApiError::rate_limited(60).status, StatusCode::TOO_MANY_REQUESTS);

        // Not found - 404
        assert_eq!(ApiError::not_found("User", "123").status, StatusCode::NOT_FOUND);
        assert_eq!(ApiError::mpc_session_not_found("abc").status, StatusCode::NOT_FOUND);

        // External service errors - 502 Bad Gateway
        assert_eq!(ApiError::rpc_error("node down").status, StatusCode::BAD_GATEWAY);

        // Database errors - 500 Internal Server Error
        let db_error = sqlx::Error::RowNotFound;
        assert_eq!(ApiError::database_error(db_error).status, StatusCode::INTERNAL_SERVER_ERROR);

        // Generic errors - 500 Internal Server Error
        assert_eq!(ApiError::internal("test").status, StatusCode::INTERNAL_SERVER_ERROR);

        // Service unavailable - 503
        assert_eq!(ApiError::service_unavailable("maintenance").status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_service_unavailable_creation() {
        let error = ApiError::service_unavailable("Database maintenance in progress");

        assert_eq!(error.code, ErrorCode::ServiceUnavailable);
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(error.message, "Database maintenance in progress");
        assert!(error.details.is_none());
        assert!(error.source.is_none());
    }

    #[test]
    fn test_error_code_values() {
        // Verify error code numeric values are in expected ranges
        assert_eq!(ErrorCode::AuthMissingToken as u16, 1000);
        assert_eq!(ErrorCode::AuthInvalidToken as u16, 1001);
        assert_eq!(ErrorCode::ValidationFailed as u16, 1100);
        assert_eq!(ErrorCode::RateLimitExceeded as u16, 1200);
        assert_eq!(ErrorCode::ResourceNotFound as u16, 1300);
        assert_eq!(ErrorCode::MpcSessionNotFound as u16, 1400);
        assert_eq!(ErrorCode::RpcError as u16, 1500);
        assert_eq!(ErrorCode::DatabaseError as u16, 1600);
        assert_eq!(ErrorCode::PolicyViolation as u16, 1700);
        assert_eq!(ErrorCode::InternalError as u16, 1800);
    }

    #[test]
    fn test_error_with_details() {
        let details = serde_json::json!({
            "field": "email",
            "reason": "invalid format"
        });

        let error = ApiError::validation_failed("Validation failed")
            .with_details(details.clone());

        assert_eq!(error.details, Some(details));
    }

    #[test]
    fn test_error_with_request_id() {
        let request_id = "req_abc123xyz";
        let error = ApiError::internal("Something went wrong")
            .with_request_id(request_id.to_string());

        assert_eq!(error.request_id, Some(request_id.to_string()));
    }

    #[test]
    fn test_rate_limited_error_includes_retry_after() {
        let retry_after = 120u64;
        let error = ApiError::rate_limited(retry_after);

        assert_eq!(error.code, ErrorCode::RateLimitExceeded);
        assert!(error.message.contains("120 seconds"));

        let details = error.details.unwrap();
        assert_eq!(details["retry_after"], retry_after);
    }

    #[test]
    fn test_not_found_error_message() {
        let error = ApiError::not_found("Transaction", "0xabc123");

        assert_eq!(error.code, ErrorCode::ResourceNotFound);
        assert!(error.message.contains("Transaction"));
        assert!(error.message.contains("0xabc123"));
    }

    #[test]
    fn test_mpc_errors() {
        let session_error = ApiError::mpc_session_not_found("session_123");
        assert_eq!(session_error.code, ErrorCode::MpcSessionNotFound);
        assert!(session_error.message.contains("session_123"));

        let protocol_error = ApiError::mpc_protocol_error("Invalid round message");
        assert_eq!(protocol_error.code, ErrorCode::MpcProtocolError);
        assert_eq!(protocol_error.message, "Invalid round message");
    }

    #[test]
    fn test_auth_errors() {
        let missing = ApiError::auth_missing_token();
        assert_eq!(missing.code, ErrorCode::AuthMissingToken);
        assert_eq!(missing.message, "Authentication token is required");

        let invalid = ApiError::auth_invalid_token("Token has been revoked");
        assert_eq!(invalid.code, ErrorCode::AuthInvalidToken);
        assert_eq!(invalid.message, "Token has been revoked");

        let forbidden = ApiError::auth_forbidden("Insufficient permissions");
        assert_eq!(forbidden.code, ErrorCode::AuthForbidden);
        assert_eq!(forbidden.message, "Insufficient permissions");
    }

    #[test]
    fn test_validation_errors() {
        let validation = ApiError::validation_failed("Invalid input");
        assert_eq!(validation.code, ErrorCode::ValidationFailed);

        let address = ApiError::invalid_address("not_an_address");
        assert_eq!(address.code, ErrorCode::InvalidAddress);
        assert!(address.message.contains("not_an_address"));

        let missing = ApiError::missing_param("amount");
        assert_eq!(missing.code, ErrorCode::MissingParameter);
        assert!(missing.message.contains("amount"));
    }

    #[test]
    fn test_error_display() {
        let error = ApiError::internal("Test error message");
        let display_output = format!("{}", error);

        assert_eq!(display_output, "Test error message");
    }

    #[test]
    fn test_error_response_structure() {
        let error = ApiError::validation_failed("Field validation failed")
            .with_request_id("req_xyz".to_string());

        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_database_error_creation() {
        let sql_error = sqlx::Error::RowNotFound;
        let error = ApiError::database_error(sql_error);

        assert_eq!(error.code, ErrorCode::DatabaseError);
        assert_eq!(error.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(error.message, "A database error occurred");
        assert!(error.source.is_some());
    }

    #[test]
    fn test_rpc_error_creation() {
        let error = ApiError::rpc_error("Failed to connect to Ethereum node");

        assert_eq!(error.code, ErrorCode::RpcError);
        assert_eq!(error.status, StatusCode::BAD_GATEWAY);
        assert!(error.message.contains("Ethereum node"));
    }

    #[test]
    fn test_error_code_serialization() {
        // Test that error codes serialize correctly
        let code = ErrorCode::AuthMissingToken;
        let serialized = serde_json::to_string(&code).unwrap();
        assert_eq!(serialized, "\"auth_missing_token\"");

        let code = ErrorCode::MpcSessionNotFound;
        let serialized = serde_json::to_string(&code).unwrap();
        assert_eq!(serialized, "\"mpc_session_not_found\"");
    }

    #[test]
    fn test_error_builder_chain() {
        let details = serde_json::json!({"key": "value"});
        let error = ApiError::internal("Chained error")
            .with_details(details.clone())
            .with_request_id("req_123".to_string());

        assert_eq!(error.message, "Chained error");
        assert_eq!(error.details, Some(details));
        assert_eq!(error.request_id, Some("req_123".to_string()));
    }

    #[test]
    fn test_all_error_constructors_compile() {
        // Ensure all error constructors are accessible and work
        let _ = ApiError::auth_missing_token();
        let _ = ApiError::auth_invalid_token("test");
        let _ = ApiError::auth_forbidden("test");
        let _ = ApiError::validation_failed("test");
        let _ = ApiError::invalid_address("0x123");
        let _ = ApiError::missing_param("param");
        let _ = ApiError::rate_limited(60);
        let _ = ApiError::not_found("Resource", "id");
        let _ = ApiError::mpc_session_not_found("session");
        let _ = ApiError::mpc_protocol_error("error");
        let _ = ApiError::rpc_error("error");
        let _ = ApiError::database_error(sqlx::Error::RowNotFound);
        let _ = ApiError::internal("error");
        let _ = ApiError::service_unavailable("error");
    }
}
