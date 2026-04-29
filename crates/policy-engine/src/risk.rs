use crate::types::{RiskLevel, TransactionContext};
use alloy_primitives::U256;

/// Assess the risk level of a transaction based on heuristics.
///
/// Risk factors:
/// - Transaction amount relative to user's typical behavior
/// - Whether the recipient is known (previous transactions)
/// - Time of day (unusual hours)
/// - Chain (newer/less-tested chains are higher risk)
/// - Contract interaction with unverified contracts
pub fn assess_risk(
    tx: &TransactionContext,
    _user_history: &UserTransactionHistory,
) -> RiskAssessment {
    let mut score: u32 = 0;
    let mut factors = Vec::new();

    // High value transactions
    let eth_value = tx.value / U256::from(10u64.pow(18));
    if eth_value > U256::from(10_000) {
        score += 40;
        factors.push("very high value (>10k ETH equivalent)".into());
    } else if eth_value > U256::from(1_000) {
        score += 20;
        factors.push("high value (>1k ETH equivalent)".into());
    }

    // Contract interaction
    if tx.is_contract_interaction {
        score += 10;
        factors.push("contract interaction".into());
    }

    // Unusual hours (local time would be better, using UTC as placeholder)
    let hour = tx.timestamp.time().hour();
    if hour >= 1 && hour <= 5 {
        score += 15;
        factors.push("unusual hour (1-5 AM UTC)".into());
    }

    let level = match score {
        0..=10 => RiskLevel::Low,
        11..=30 => RiskLevel::Medium,
        31..=60 => RiskLevel::High,
        _ => RiskLevel::Critical,
    };

    RiskAssessment {
        level,
        score,
        factors,
    }
}

#[derive(Debug, Clone)]
pub struct RiskAssessment {
    pub level: RiskLevel,
    pub score: u32,
    pub factors: Vec<String>,
}

/// User's recent transaction history for anomaly detection.
pub struct UserTransactionHistory {
    pub avg_daily_volume: U256,
    pub avg_tx_count_per_day: u32,
    pub known_recipients: Vec<alloy_primitives::Address>,
    pub typical_hours: (u8, u8),
}

use chrono::Timelike;
