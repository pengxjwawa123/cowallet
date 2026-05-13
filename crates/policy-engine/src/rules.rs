use crate::types::{Decision, Policy, PolicyAction, Rule, TransactionContext};

/// Evaluate a transaction against all active policies.
///
/// Policies are evaluated in priority order (highest first).
/// The first matching policy determines the action.
/// If no policy matches, the default is RequireBiometric.
pub fn evaluate(tx: &TransactionContext, policies: &[Policy]) -> Decision {
    let mut sorted: Vec<&Policy> = policies.iter().filter(|p| p.enabled).collect();
    sorted.sort_by(|a, b| b.priority.cmp(&a.priority));

    for policy in sorted {
        if all_rules_match(&policy.rules, tx) {
            return Decision {
                allowed: !matches!(policy.action, PolicyAction::Deny { .. }),
                action: policy.action.clone(),
                matched_policy: Some(policy.id),
                reason: format!("matched policy: {}", policy.name),
            };
        }
    }

    Decision {
        allowed: true,
        action: PolicyAction::RequireBiometric,
        matched_policy: None,
        reason: "no policy matched, default: require biometric".into(),
    }
}

fn all_rules_match(rules: &[Rule], tx: &TransactionContext) -> bool {
    rules.iter().all(|rule| rule_matches(rule, tx))
}

fn rule_matches(rule: &Rule, tx: &TransactionContext) -> bool {
    match rule {
        Rule::ExceedsAmount { token, limit } => {
            let token_matches = match token {
                Some(t) => tx.token.as_deref() == Some(t.as_str()),
                None => true,
            };
            token_matches && tx.value > *limit
        }

        Rule::WhitelistOnly { addresses } => !addresses.contains(&tx.to),

        Rule::BlacklistCheck { addresses } => addresses.contains(&tx.to),

        Rule::ChainRestriction { allowed_chains } => !allowed_chains.contains(&tx.chain_id),

        Rule::ContractInteraction { allow_unknown } => tx.is_contract_interaction && !allow_unknown,

        Rule::TimeWindow {
            start_hour,
            end_hour,
        } => {
            let hour = tx.timestamp.time().hour() as u8;
            if start_hour <= end_hour {
                hour < *start_hour || hour >= *end_hour
            } else {
                hour < *start_hour && hour >= *end_hour
            }
        }

        Rule::DailyLimit { token, limit } => {
            let token_matches = match token {
                Some(t) => tx.token.as_deref() == Some(t.as_str()),
                None => true,
            };
            match &tx.history {
                Some(h) if token_matches => (h.daily_total + tx.value) > *limit,
                _ => false,
            }
        }

        Rule::RateLimit { max_tx, window_secs } => {
            match &tx.history {
                Some(h) if h.window_secs == *window_secs => h.window_tx_count >= *max_tx,
                _ => false,
            }
        }
    }
}

use chrono::Timelike;

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::Address;
    use chrono::Utc;
    use uuid::Uuid;

    fn test_tx(value: u128, to: Address) -> TransactionContext {
        TransactionContext {
            user_id: "user-1".into(),
            from: Address::ZERO,
            to,
            value: alloy_primitives::U256::from(value),
            token: None,
            chain_id: 1,
            is_contract_interaction: false,
            timestamp: Utc::now(),
            history: None,
        }
    }

    #[test]
    fn test_blacklist_blocks() {
        let bad_addr: Address = "0x0000000000000000000000000000000000000BAD"
            .parse()
            .unwrap();

        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "block bad addresses".into(),
            description: "deny transactions to known bad addresses".into(),
            rules: vec![Rule::BlacklistCheck {
                addresses: vec![bad_addr],
            }],
            action: PolicyAction::Deny {
                reason: "blacklisted address".into(),
            },
            enabled: true,
            priority: 100,
        }];

        let tx = test_tx(1000, bad_addr);
        let decision = evaluate(&tx, &policies);
        assert!(!decision.allowed);
    }

    #[test]
    fn test_no_policy_defaults_to_biometric() {
        let tx = test_tx(100, Address::ZERO);
        let decision = evaluate(&tx, &[]);
        assert!(decision.allowed);
        assert!(matches!(decision.action, PolicyAction::RequireBiometric));
    }

    #[test]
    fn test_amount_limit_allows_below_threshold() {
        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "high value check".into(),
            description: "require approval for >1 ETH".into(),
            rules: vec![Rule::ExceedsAmount {
                token: None,
                limit: alloy_primitives::U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            }],
            action: PolicyAction::RequireApproval {
                approvers: vec!["manager".into()],
                threshold: 1,
            },
            enabled: true,
            priority: 100,
        }];

        // 0.5 ETH - should not trigger
        let tx = test_tx(500_000_000_000_000_000, Address::ZERO);
        let decision = evaluate(&tx, &policies);
        assert!(decision.allowed);
        assert!(matches!(decision.action, PolicyAction::RequireBiometric));
        assert!(decision.matched_policy.is_none());
    }

    #[test]
    fn test_amount_limit_blocks_above_threshold() {
        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "high value check".into(),
            description: "require approval for >1 ETH".into(),
            rules: vec![Rule::ExceedsAmount {
                token: None,
                limit: alloy_primitives::U256::from(1_000_000_000_000_000_000u128), // 1 ETH
            }],
            action: PolicyAction::RequireApproval {
                approvers: vec!["manager".into()],
                threshold: 1,
            },
            enabled: true,
            priority: 100,
        }];

        // 2 ETH - should trigger approval requirement
        let tx = test_tx(2_000_000_000_000_000_000, Address::ZERO);
        let decision = evaluate(&tx, &policies);
        assert!(decision.allowed);
        assert!(matches!(
            decision.action,
            PolicyAction::RequireApproval { .. }
        ));
        assert!(decision.matched_policy.is_some());
    }

    #[test]
    fn test_whitelist_only_allows_whitelisted() {
        let whitelisted_addr: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();
        let other_addr: Address = "0x0987654321098765432109876543210987654321"
            .parse()
            .unwrap();

        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "whitelist only".into(),
            description: "deny if not on whitelist".into(),
            rules: vec![Rule::WhitelistOnly {
                addresses: vec![whitelisted_addr],
            }],
            action: PolicyAction::Deny {
                reason: "recipient not whitelisted".into(),
            },
            enabled: true,
            priority: 100,
        }];

        // Whitelisted address - should be allowed (no match)
        let tx_whitelisted = test_tx(1000, whitelisted_addr);
        let decision = evaluate(&tx_whitelisted, &policies);
        assert!(decision.allowed);

        // Non-whitelisted address - should be denied
        let tx_other = test_tx(1000, other_addr);
        let decision = evaluate(&tx_other, &policies);
        assert!(!decision.allowed);
    }

    #[test]
    fn test_chain_restriction() {
        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "mainnet only".into(),
            description: "deny testnets".into(),
            rules: vec![Rule::ChainRestriction {
                allowed_chains: vec![1, 8453, 42161], // Mainnet chains only
            }],
            action: PolicyAction::Deny {
                reason: "testnet not allowed".into(),
            },
            enabled: true,
            priority: 100,
        }];

        // Mainnet (chain_id=1) - should be allowed
        let mut tx_mainnet = test_tx(1000, Address::ZERO);
        tx_mainnet.chain_id = 1;
        let decision = evaluate(&tx_mainnet, &policies);
        assert!(decision.allowed);

        // Testnet (chain_id=84532) - should be denied
        let mut tx_testnet = test_tx(1000, Address::ZERO);
        tx_testnet.chain_id = 84532;
        let decision = evaluate(&tx_testnet, &policies);
        assert!(!decision.allowed);
    }

    #[test]
    fn test_time_window_restriction() {
        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "business hours only".into(),
            description: "deny outside 9-17".into(),
            rules: vec![Rule::TimeWindow {
                start_hour: 9,
                end_hour: 17,
            }],
            action: PolicyAction::Deny {
                reason: "outside business hours".into(),
            },
            enabled: true,
            priority: 100,
        }];

        // Create tx with timestamp at 3 AM UTC (outside window)
        let mut tx_night = test_tx(1000, Address::ZERO);
        tx_night.timestamp = chrono::DateTime::parse_from_rfc3339("2024-01-15T03:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let decision = evaluate(&tx_night, &policies);
        assert!(!decision.allowed);

        // Create tx with timestamp at 12 PM UTC (inside window)
        let mut tx_day = test_tx(1000, Address::ZERO);
        tx_day.timestamp = chrono::DateTime::parse_from_rfc3339("2024-01-15T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let decision = evaluate(&tx_day, &policies);
        assert!(decision.allowed);
    }

    #[test]
    fn test_contract_interaction_check() {
        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "no unknown contracts".into(),
            description: "deny contract interactions".into(),
            rules: vec![Rule::ContractInteraction {
                allow_unknown: false,
            }],
            action: PolicyAction::Deny {
                reason: "unknown contract interaction".into(),
            },
            enabled: true,
            priority: 100,
        }];

        // Regular transfer - should be allowed
        let tx_eoa = test_tx(1000, Address::ZERO);
        let decision = evaluate(&tx_eoa, &policies);
        assert!(decision.allowed);

        // Contract interaction - should be denied
        let mut tx_contract = test_tx(1000, Address::ZERO);
        tx_contract.is_contract_interaction = true;
        let decision = evaluate(&tx_contract, &policies);
        assert!(!decision.allowed);
    }

    #[test]
    fn test_policy_priority_ordering() {
        let high_priority_policy = Policy {
            id: Uuid::new_v4(),
            name: "high priority".into(),
            description: "high".into(),
            rules: vec![Rule::ExceedsAmount {
                token: None,
                limit: alloy_primitives::U256::from(100),
            }],
            action: PolicyAction::Deny {
                reason: "high priority deny".into(),
            },
            enabled: true,
            priority: 200,
        };

        let low_priority_policy = Policy {
            id: Uuid::new_v4(),
            name: "low priority".into(),
            description: "low".into(),
            rules: vec![Rule::ExceedsAmount {
                token: None,
                limit: alloy_primitives::U256::from(100),
            }],
            action: PolicyAction::Approve,
            enabled: true,
            priority: 50,
        };

        let policies = vec![low_priority_policy, high_priority_policy.clone()];

        let tx = test_tx(200, Address::ZERO);
        let decision = evaluate(&tx, &policies);

        // High priority policy should match first
        assert!(!decision.allowed);
        assert_eq!(decision.matched_policy, Some(high_priority_policy.id));
    }

    #[test]
    fn test_disabled_policy_ignored() {
        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "disabled policy".into(),
            description: "should be ignored".into(),
            rules: vec![Rule::ExceedsAmount {
                token: None,
                limit: alloy_primitives::U256::from(0),
            }],
            action: PolicyAction::Deny {
                reason: "should not trigger".into(),
            },
            enabled: false, // Disabled
            priority: 100,
        }];

        let tx = test_tx(1000, Address::ZERO);
        let decision = evaluate(&tx, &policies);

        // Should use default behavior since policy is disabled
        assert!(decision.allowed);
        assert!(decision.matched_policy.is_none());
    }

    #[test]
    fn test_multiple_rules_all_must_match() {
        let addr: Address = "0x1234567890123456789012345678901234567890"
            .parse()
            .unwrap();

        let policies = vec![Policy {
            id: Uuid::new_v4(),
            name: "multi-rule policy".into(),
            description: "requires both high value AND specific address".into(),
            rules: vec![
                Rule::ExceedsAmount {
                    token: None,
                    limit: alloy_primitives::U256::from(1_000_000),
                },
                Rule::BlacklistCheck {
                    addresses: vec![addr],
                },
            ],
            action: PolicyAction::Deny {
                reason: "high value to blacklisted address".into(),
            },
            enabled: true,
            priority: 100,
        }];

        // High value but different address - no match
        let tx1 = test_tx(2_000_000, Address::ZERO);
        let decision = evaluate(&tx1, &policies);
        assert!(decision.allowed);

        // Correct address but low value - no match
        let tx2 = test_tx(100, addr);
        let decision = evaluate(&tx2, &policies);
        assert!(decision.allowed);

        // Both conditions met - should match and deny
        let tx3 = test_tx(2_000_000, addr);
        let decision = evaluate(&tx3, &policies);
        assert!(!decision.allowed);
    }
}
