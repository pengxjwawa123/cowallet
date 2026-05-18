//! Simple USD-based transfer limits for policy enforcement.
//!
//! Evaluates a transaction context against per-user limits and returns
//! a result indicating whether the transaction is allowed, warned, or blocked.

use serde::{Deserialize, Serialize};

/// Per-user configurable limits (stored in DB).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserLimits {
    /// Max USD value for a single transfer.
    pub single_limit_usd: f64,
    /// Max cumulative USD value in a rolling 24h window.
    pub daily_limit_usd: f64,
}

impl Default for UserLimits {
    fn default() -> Self {
        Self {
            single_limit_usd: 500.0,
            daily_limit_usd: 2000.0,
        }
    }
}

/// Context for evaluating a transfer against policy limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TxContext {
    pub from: String,
    pub to: String,
    pub value_usd: f64,
    pub token: String,
    pub chain_id: u64,
    pub is_new_recipient: bool,
    pub daily_total_usd: f64,
}

/// Result of policy evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyResult {
    pub allowed: bool,
    pub warnings: Vec<String>,
    pub requires_extra_confirmation: bool,
    /// If not allowed, the reason and limit that was violated.
    pub violation: Option<PolicyViolation>,
}

/// Details of a policy violation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyViolation {
    pub reason: String,
    pub limit: String,
}

/// Evaluate a transfer against the user's configured limits.
pub fn evaluate(ctx: &TxContext, limits: &UserLimits) -> PolicyResult {
    let mut warnings: Vec<String> = Vec::new();

    // Check single transfer limit
    if ctx.value_usd > limits.single_limit_usd {
        return PolicyResult {
            allowed: false,
            warnings: Vec::new(),
            requires_extra_confirmation: false,
            violation: Some(PolicyViolation {
                reason: format!(
                    "Single transfer ${:.2} exceeds limit of ${:.2}",
                    ctx.value_usd, limits.single_limit_usd
                ),
                limit: format!("${:.2}", limits.single_limit_usd),
            }),
        };
    }

    // Check daily cumulative limit
    let new_daily_total = ctx.daily_total_usd + ctx.value_usd;
    if new_daily_total > limits.daily_limit_usd {
        return PolicyResult {
            allowed: false,
            warnings: Vec::new(),
            requires_extra_confirmation: false,
            violation: Some(PolicyViolation {
                reason: format!(
                    "Daily total ${:.2} (including this ${:.2}) exceeds limit of ${:.2}",
                    new_daily_total, ctx.value_usd, limits.daily_limit_usd
                ),
                limit: format!("${:.2}", limits.daily_limit_usd),
            }),
        };
    }

    // New recipient warning
    if ctx.is_new_recipient {
        warnings.push(format!(
            "First transfer to address {}. Please verify the recipient.",
            ctx.to
        ));
    }

    // High-value warning (>80% of single limit)
    if ctx.value_usd > limits.single_limit_usd * 0.8 {
        warnings.push(format!(
            "Transfer ${:.2} is close to your single-transfer limit of ${:.2}.",
            ctx.value_usd, limits.single_limit_usd
        ));
    }

    let requires_extra_confirmation = ctx.is_new_recipient || ctx.value_usd > limits.single_limit_usd * 0.5;

    PolicyResult {
        allowed: true,
        warnings,
        requires_extra_confirmation,
        violation: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ctx(value_usd: f64, daily_total: f64, is_new: bool) -> TxContext {
        TxContext {
            from: "0xaaa".into(),
            to: "0xbbb".into(),
            value_usd,
            token: "ETH".into(),
            chain_id: 1,
            is_new_recipient: is_new,
            daily_total_usd: daily_total,
        }
    }

    #[test]
    fn test_within_limits() {
        let ctx = test_ctx(100.0, 0.0, false);
        let limits = UserLimits::default();
        let result = evaluate(&ctx, &limits);
        assert!(result.allowed);
        assert!(result.violation.is_none());
        assert!(result.warnings.is_empty());
    }

    #[test]
    fn test_exceeds_single_limit() {
        let ctx = test_ctx(600.0, 0.0, false);
        let limits = UserLimits::default();
        let result = evaluate(&ctx, &limits);
        assert!(!result.allowed);
        assert!(result.violation.is_some());
        assert!(result.violation.unwrap().reason.contains("Single transfer"));
    }

    #[test]
    fn test_exceeds_daily_limit() {
        let ctx = test_ctx(400.0, 1700.0, false);
        let limits = UserLimits::default();
        let result = evaluate(&ctx, &limits);
        assert!(!result.allowed);
        assert!(result.violation.is_some());
        assert!(result.violation.unwrap().reason.contains("Daily total"));
    }

    #[test]
    fn test_new_recipient_warning() {
        let ctx = test_ctx(100.0, 0.0, true);
        let limits = UserLimits::default();
        let result = evaluate(&ctx, &limits);
        assert!(result.allowed);
        assert!(result.warnings.iter().any(|w| w.contains("First transfer")));
        assert!(result.requires_extra_confirmation);
    }

    #[test]
    fn test_high_value_warning() {
        let ctx = test_ctx(450.0, 0.0, false);
        let limits = UserLimits::default();
        let result = evaluate(&ctx, &limits);
        assert!(result.allowed);
        assert!(result.warnings.iter().any(|w| w.contains("close to your single-transfer limit")));
    }

    #[test]
    fn test_custom_limits() {
        let ctx = test_ctx(1500.0, 0.0, false);
        let limits = UserLimits {
            single_limit_usd: 2000.0,
            daily_limit_usd: 10000.0,
        };
        let result = evaluate(&ctx, &limits);
        assert!(result.allowed);
    }
}
