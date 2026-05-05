//! Audit logging middleware for sensitive operations.
//!
//! Logs: signing attempts, policy changes, authentication events
//! Fields: user_id, action, timestamp, ip, device_attestation, result, duration, user_agent

use axum::{
    body::Body,
    http::{Request, Response},
    middleware::Next,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;
use std::time::Instant;

/// Audit log entry for sensitive operations
#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct AuditLog {
    pub id: Uuid,
    pub user_id: Uuid,
    pub action: String,
    pub timestamp: DateTime<Utc>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub device_attestation: Option<String>,
    pub result: AuditResult,
    pub duration_ms: Option<i64>,
    pub details: Option<serde_json::Value>,
}

/// Result of an audited operation
#[derive(Debug, Clone, Copy, Serialize, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AuditResult {
    Success,
    Failed,
    Denied,
    Pending,
}

impl std::fmt::Display for AuditResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditResult::Success => write!(f, "success"),
            AuditResult::Failed => write!(f, "failed"),
            AuditResult::Denied => write!(f, "denied"),
            AuditResult::Pending => write!(f, "pending"),
        }
    }
}

impl AuditResult {
    /// Convert from status code to audit result
    pub fn from_status_code(status: axum::http::StatusCode) -> Self {
        if status.is_success() {
            AuditResult::Success
        } else if status.is_client_error() {
            AuditResult::Denied
        } else {
            AuditResult::Failed
        }
    }
}

/// Common audit actions for consistent logging
pub mod audit_actions {
    pub const MPC_SIGN_ATTEMPT: &str = "mpc.sign_attempt";
    pub const MPC_SIGN_COMPLETE: &str = "mpc.sign_complete";
    pub const MPC_SESSION_CREATE: &str = "mpc.session_create";
    pub const MPC_SESSION_ABORT: &str = "mpc.session_abort";

    pub const AUTH_LOGIN: &str = "auth.login";
    pub const AUTH_LOGOUT: &str = "auth.logout";
    pub const AUTH_DEVICE_VERIFY: &str = "auth.device_verify";

    pub const POLICY_CREATE: &str = "policy.create";
    pub const POLICY_UPDATE: &str = "policy.update";
    pub const POLICY_DELETE: &str = "policy.delete";

    pub const TX_SUBMIT: &str = "tx.submit";
    pub const TX_BROADCAST: &str = "tx.broadcast";

    pub const KEY_BACKUP: &str = "key.backup";
    pub const KEY_ROTATE: &str = "key.rotate";
}

/// Audit log service for creating and querying audit logs
#[derive(Clone)]
pub struct AuditLogger {
    pool: Option<PgPool>,
}

impl AuditLogger {
    /// Create a new audit logger
    pub fn new(pool: Option<PgPool>) -> Self {
        Self { pool }
    }

    /// Create a no-op audit logger (for when DB is unavailable)
    pub fn noop() -> Self {
        Self { pool: None }
    }

    /// Log an operation with minimal fields
    pub async fn log(
        &self,
        user_id: Uuid,
        action: &str,
        result: AuditResult,
        ip_address: Option<String>,
    ) -> Result<(), sqlx::Error> {
        self.log_with_details(
            user_id,
            action,
            result,
            ip_address,
            None,
            None,
            None,
            None,
        )
        .await
    }

    /// Log with additional details including duration
    pub async fn log_with_details(
        &self,
        user_id: Uuid,
        action: &str,
        result: AuditResult,
        ip_address: Option<String>,
        user_agent: Option<String>,
        device_attestation: Option<String>,
        duration_ms: Option<i64>,
        details: Option<serde_json::Value>,
    ) -> Result<(), sqlx::Error> {
        tracing::info!(
            "AUDIT: user={}, action={}, result={}, ip={:?}, ua={:?}, duration={:?}ms",
            user_id,
            action,
            result,
            ip_address,
            user_agent,
            duration_ms
        );

        if let Some(pool) = &self.pool {
            sqlx::query(
                "INSERT INTO audit_logs
                 (user_id, action, result, ip_address, user_agent, device_attestation, duration_ms, details)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
            )
            .bind(user_id)
            .bind(action)
            .bind(result.to_string())
            .bind(ip_address)
            .bind(user_agent)
            .bind(device_attestation)
            .bind(duration_ms)
            .bind(details)
            .execute(pool)
            .await?;
        }

        Ok(())
    }

    /// Get recent audit logs for a user
    pub async fn get_user_logs(
        &self,
        user_id: Uuid,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<AuditLog>, sqlx::Error> {
        tracing::debug!("AUDIT: get_user_logs for user={}", user_id);

        if let Some(pool) = &self.pool {
            let logs = sqlx::query_as::<_, AuditLog>(
                "SELECT id, user_id, action, timestamp, ip_address, user_agent,
                        device_attestation, result, duration_ms, details
                 FROM audit_logs
                 WHERE user_id = $1
                 ORDER BY timestamp DESC
                 LIMIT $2 OFFSET $3",
            )
            .bind(user_id)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await?;

            Ok(logs)
        } else {
            Ok(Vec::new())
        }
    }

    /// Get audit logs filtered by action type
    pub async fn get_logs_by_action(
        &self,
        user_id: Uuid,
        action: &str,
        limit: i64,
    ) -> Result<Vec<AuditLog>, sqlx::Error> {
        if let Some(pool) = &self.pool {
            let logs = sqlx::query_as::<_, AuditLog>(
                "SELECT id, user_id, action, timestamp, ip_address, user_agent,
                        device_attestation, result, duration_ms, details
                 FROM audit_logs
                 WHERE user_id = $1 AND action = $2
                 ORDER BY timestamp DESC
                 LIMIT $3",
            )
            .bind(user_id)
            .bind(action)
            .bind(limit)
            .fetch_all(pool)
            .await?;

            Ok(logs)
        } else {
            Ok(Vec::new())
        }
    }
}

/// Axum middleware for request auditing
/// Wraps routes and logs sensitive operations
/// Note: This middleware currently only logs to tracing (database logging
/// is done per-route for authenticated operations)
pub async fn audit_middleware(request: Request<Body>, next: Next) -> Response<Body> {
    let start = Instant::now();

    // Capture request details before processing
    let path = request.uri().path().to_string();
    let method = request.method().to_string();
    let ip_address = request
        .headers()
        .get("X-Forwarded-For")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim().to_string());

    tracing::debug!("Audit middleware: {} {} from {:?}", method, path, ip_address);

    // Process request
    let response = next.run(request).await;

    let duration_ms = start.elapsed().as_millis() as i64;
    let status = response.status();
    let result = AuditResult::from_status_code(status);

    tracing::debug!(
        "Audit result: path={}, status={}, result={}, duration={}ms",
        path,
        status,
        result,
        duration_ms
    );

    response
}

/// Map route paths to canonical audit action names (reduces cardinality)
fn map_path_to_action(path: &str, method: &str) -> String {
    let path = path.trim_start_matches("/api/v1/");

    if path.starts_with("mpc/") {
        if path.contains("session") {
            return "mpc.session".to_string();
        }
        if path.contains("sign") {
            return "mpc.sign".to_string();
        }
        return "mpc.other".to_string();
    }
    if path.starts_with("tx/") {
        if path.contains("submit") {
            return "tx.submit".to_string();
        }
        if path.contains("history") {
            return "tx.history".to_string();
        }
        return "tx.other".to_string();
    }
    if path.starts_with("policy/") {
        return format!("policy.{}", method.to_lowercase());
    }
    if path.starts_with("auth/") {
        if path.contains("login") {
            return "auth.login".to_string();
        }
        if path.contains("register") {
            return "auth.register".to_string();
        }
        return "auth.other".to_string();
    }
    if path.starts_with("yield/") {
        return "yield.query".to_string();
    }
    if path.starts_with("ai/") {
        return "ai.query".to_string();
    }

    format!("{}.{}", method.to_lowercase(), path.replace('/', "."))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_result_display() {
        assert_eq!(AuditResult::Success.to_string(), "success");
        assert_eq!(AuditResult::Failed.to_string(), "failed");
        assert_eq!(AuditResult::Denied.to_string(), "denied");
        assert_eq!(AuditResult::Pending.to_string(), "pending");
    }
}
