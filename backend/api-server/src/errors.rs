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
