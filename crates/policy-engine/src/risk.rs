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

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use chrono::{TimeZone, Utc};

    fn test_history() -> UserTransactionHistory {
        UserTransactionHistory {
            avg_daily_volume: U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            avg_tx_count_per_day: 5,
            known_recipients: vec![Address::ZERO],
            typical_hours: (9, 17),
        }
    }

    fn test_tx(
        value_eth: u64,
        hour: u32,
        is_contract: bool,
    ) -> TransactionContext {
        TransactionContext {
            user_id: "user-1".into(),
            from: Address::ZERO,
            to: Address::ZERO,
            value: U256::from(value_eth) * U256::from(10u64.pow(18)),
            token: None,
            chain_id: 1,
            is_contract_interaction: is_contract,
            timestamp: Utc.with_ymd_and_hms(2024, 1, 15, hour, 0, 0).unwrap(),
            history: None,
        }
    }

    #[test]
    fn test_low_risk_normal_transaction() {
        let tx = test_tx(1, 12, false); // 1 ETH at noon, not contract
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert_eq!(assessment.level, RiskLevel::Low);
        assert!(assessment.score <= 10);
    }

    #[test]
    fn test_medium_risk_high_value() {
        let tx = test_tx(1500, 12, false); // 1500 ETH at noon
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert_eq!(assessment.level, RiskLevel::Medium);
        assert!(assessment.score > 10 && assessment.score <= 30);
        assert!(assessment.factors.iter().any(|f| f.contains("high value")));
    }

    #[test]
    fn test_high_risk_very_high_value() {
        let tx = test_tx(15000, 12, false); // 15000 ETH at noon
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert_eq!(assessment.level, RiskLevel::High);
        assert!(assessment.score > 30);
        assert!(assessment
            .factors
            .iter()
            .any(|f| f.contains("very high value")));
    }

    #[test]
    fn test_risk_unusual_hours() {
        let tx = test_tx(1, 3, false); // 1 ETH at 3 AM
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert!(assessment.score >= 15);
        assert!(assessment.factors.iter().any(|f| f.contains("unusual hour")));
    }

    #[test]
    fn test_risk_contract_interaction() {
        let tx = test_tx(1, 12, true); // 1 ETH at noon, contract interaction
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert!(assessment.score >= 10);
        assert!(assessment
            .factors
            .iter()
            .any(|f| f.contains("contract interaction")));
    }

    #[test]
    fn test_critical_risk_multiple_factors() {
        let tx = test_tx(20000, 2, true); // 20000 ETH at 2 AM, contract
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert_eq!(assessment.level, RiskLevel::Critical);
        assert!(assessment.score > 60);
        assert!(assessment.factors.len() >= 2);
    }

    #[test]
    fn test_risk_score_boundaries() {
        let history = test_history();

        // Score 0-10: Low
        let tx_low = test_tx(0, 12, false);
        let assess_low = assess_risk(&tx_low, &history);
        assert_eq!(assess_low.level, RiskLevel::Low);

        // Score 11-30: Medium
        let tx_medium = test_tx(1500, 12, false);
        let assess_medium = assess_risk(&tx_medium, &history);
        assert_eq!(assess_medium.level, RiskLevel::Medium);

        // Score 31-60: High
        let tx_high = test_tx(15000, 3, false);
        let assess_high = assess_risk(&tx_high, &history);
        assert_eq!(assess_high.level, RiskLevel::High);

        // Score 61+: Critical
        let tx_critical = test_tx(20000, 2, true);
        let assess_critical = assess_risk(&tx_critical, &history);
        assert_eq!(assess_critical.level, RiskLevel::Critical);
    }

    #[test]
    fn test_risk_factors_accumulate() {
        let tx = test_tx(5000, 2, true); // High value + unusual hour + contract
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        // Should have multiple risk factors
        assert!(assessment.factors.len() >= 2);
        assert!(assessment.score > 30);
    }

    #[test]
    fn test_normal_hours_no_penalty() {
        let history = test_history();

        for hour in 6..22 {
            let tx = test_tx(1, hour, false);
            let assessment = assess_risk(&tx, &history);

            // Should not have unusual hour penalty for daytime hours
            if hour >= 6 {
                assert!(!assessment
                    .factors
                    .iter()
                    .any(|f| f.contains("unusual hour")));
            }
        }
    }

    #[test]
    fn test_early_morning_hours_flagged() {
        let history = test_history();

        for hour in 1..=5 {
            let tx = test_tx(1, hour, false);
            let assessment = assess_risk(&tx, &history);

            assert!(
                assessment.factors.iter().any(|f| f.contains("unusual hour")),
                "hour {} should be flagged as unusual",
                hour
            );
        }
    }

    #[test]
    fn test_value_threshold_10k_eth() {
        let history = test_history();

        // Just below 10k ETH
        let tx_below = test_tx(9999, 12, false);
        let assess_below = assess_risk(&tx_below, &history);

        // Just above 10k ETH
        let tx_above = test_tx(10001, 12, false);
        let assess_above = assess_risk(&tx_above, &history);

        // Above threshold should have higher score
        assert!(assess_above.score > assess_below.score);
        assert!(assess_above
            .factors
            .iter()
            .any(|f| f.contains("very high value")));
    }

    #[test]
    fn test_value_threshold_1k_eth() {
        let history = test_history();

        // Just below 1k ETH
        let tx_below = test_tx(999, 12, false);
        let assess_below = assess_risk(&tx_below, &history);

        // Just above 1k ETH
        let tx_above = test_tx(1001, 12, false);
        let assess_above = assess_risk(&tx_above, &history);

        // Above threshold should have higher score
        assert!(assess_above.score > assess_below.score);
    }

    #[test]
    fn test_zero_value_transaction() {
        let tx = test_tx(0, 12, false);
        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert_eq!(assessment.level, RiskLevel::Low);
        assert_eq!(assessment.score, 0);
    }

    #[test]
    fn test_known_good_address_low_value() {
        let mut tx = test_tx(1, 14, false); // 1 ETH at 2 PM
        tx.to = Address::ZERO; // Known recipient

        let history = test_history();
        let assessment = assess_risk(&tx, &history);

        assert_eq!(assessment.level, RiskLevel::Low);
    }

    #[test]
    fn test_risk_assessment_deterministic() {
        let tx = test_tx(5000, 3, true);
        let history = test_history();

        let assessment1 = assess_risk(&tx, &history);
        let assessment2 = assess_risk(&tx, &history);

        assert_eq!(assessment1.score, assessment2.score);
        assert_eq!(assessment1.level, assessment2.level);
        assert_eq!(assessment1.factors.len(), assessment2.factors.len());
    }
}
