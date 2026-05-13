use alloy_primitives::{Address, U256};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Pre-computed aggregates from recent transaction history.
/// Populated by the caller (backend) before policy evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransactionHistory {
    /// Total value sent in the last 24 hours (in wei/smallest unit).
    pub daily_total: U256,
    /// Number of transactions in the specified rate-limit window.
    pub window_tx_count: u32,
    /// Duration of the window in seconds (matches `RateLimit::window_secs`).
    pub window_secs: u64,
}

/// A transaction context to evaluate against policies.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionContext {
    pub user_id: String,
    pub from: Address,
    pub to: Address,
    pub value: U256,
    pub token: Option<String>,
    pub chain_id: u64,
    pub is_contract_interaction: bool,
    pub timestamp: DateTime<Utc>,
    /// Recent history for daily-limit / rate-limit rules. None = rules skipped.
    #[serde(default)]
    pub history: Option<TransactionHistory>,
}

/// A policy is a named collection of rules with an action.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub rules: Vec<Rule>,
    pub action: PolicyAction,
    pub enabled: bool,
    pub priority: u32,
}

/// Individual rules that evaluate a transaction context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Rule {
    ExceedsAmount { token: Option<String>, limit: U256 },

    DailyLimit { token: Option<String>, limit: U256 },

    RateLimit { max_tx: u32, window_secs: u64 },

    WhitelistOnly { addresses: Vec<Address> },

    BlacklistCheck { addresses: Vec<Address> },

    TimeWindow { start_hour: u8, end_hour: u8 },

    ChainRestriction { allowed_chains: Vec<u64> },

    ContractInteraction { allow_unknown: bool },
}

/// Action to take when a policy's rules match.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PolicyAction {
    Approve,

    Deny {
        reason: String,
    },

    RequireApproval {
        approvers: Vec<String>,
        threshold: u32,
    },

    RequireBiometric,

    Delay {
        seconds: u64,
    },
}

/// The result of evaluating all policies against a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Decision {
    pub allowed: bool,
    pub action: PolicyAction,
    pub matched_policy: Option<Uuid>,
    pub reason: String,
}

/// Risk level classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}
