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

        // These rules require historical data — delegate to the risk module.
        Rule::DailyLimit { .. } | Rule::RateLimit { .. } => {
            // TODO: Query recent transaction history to check limits
            false
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
}
