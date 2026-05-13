use serde::{Deserialize, Serialize};

/// Intent classification result from the local classifier or Claude API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Intent {
    pub kind: IntentKind,
    pub confidence: f32,
    pub entities: Vec<Entity>,
    pub raw_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum IntentKind {
    CheckBalance,
    Transfer,
    SpendingSummary,
    PriceQuery,
    YieldSearch,
    ContractExplain,
    GeneralQuestion,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub kind: EntityKind,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EntityKind {
    Amount,
    Token,
    Address,
    Contact,
    Chain,
    TimePeriod,
}

/// Simple regex-based intent classifier (mirrors the prototype's 5 rules).
/// Will be replaced by a local ML model in Phase 4.
pub fn classify(text: &str) -> Intent {
    let lower = text.to_lowercase();
    let text_owned = text.to_string();

    if lower.contains("余额")
        || lower.contains("总共")
        || lower.contains("balance")
        || lower.contains("total")
    {
        return Intent {
            kind: IntentKind::CheckBalance,
            confidence: 0.8,
            entities: vec![],
            raw_text: text_owned,
        };
    }

    if lower.contains("转") || lower.contains("send") || lower.contains("transfer") {
        let entities = extract_transfer_entities(text);
        return Intent {
            kind: IntentKind::Transfer,
            confidence: 0.7,
            entities,
            raw_text: text_owned,
        };
    }

    if lower.contains("花了") || lower.contains("支出") || lower.contains("spend") {
        let entities = extract_time_period(text);
        return Intent {
            kind: IntentKind::SpendingSummary,
            confidence: 0.8,
            entities,
            raw_text: text_owned,
        };
    }

    if lower.contains("价格") || lower.contains("price") || lower.contains("多少钱") {
        let entities = extract_token(text);
        return Intent {
            kind: IntentKind::PriceQuery,
            confidence: 0.7,
            entities,
            raw_text: text_owned,
        };
    }

    if lower.contains("理财")
        || lower.contains("利息")
        || lower.contains("yield")
        || lower.contains("earn")
    {
        return Intent {
            kind: IntentKind::YieldSearch,
            confidence: 0.7,
            entities: vec![],
            raw_text: text_owned,
        };
    }

    Intent {
        kind: IntentKind::Unknown,
        confidence: 0.0,
        entities: vec![],
        raw_text: text_owned,
    }
}

/// Extract transfer-related entities (amount, token, address/contact)
fn extract_transfer_entities(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    // Extract amount: numbers with optional decimals
    let amount_patterns = [
        r"\b(\d+\.?\d*)\s*(ETH|USDC|USDT|BNB|MATIC|ARB|OP)",
        r"\b(\d+\.?\d*)\s+(?:个|枚)",
        r"\b(\d+\.?\d*)",
    ];

    for pattern in &amount_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(amount) = caps.get(1) {
                    entities.push(Entity {
                        kind: EntityKind::Amount,
                        value: amount.as_str().to_string(),
                    });
                    break;
                }
            }
        }
    }

    // Extract token symbol
    let token_pattern = r"\b(ETH|USDC|USDT|BNB|MATIC|ARB|OP|以太坊|以太币)\b";
    if let Ok(re) = regex::Regex::new(token_pattern) {
        if let Some(caps) = re.captures(text) {
            if let Some(token) = caps.get(1) {
                let normalized = match token.as_str() {
                    "以太坊" | "以太币" => "ETH",
                    other => other,
                };
                entities.push(Entity {
                    kind: EntityKind::Token,
                    value: normalized.to_string(),
                });
            }
        }
    }

    // Extract Ethereum address (0x followed by 40 hex chars)
    let address_pattern = r"(0x[a-fA-F0-9]{40})\b";
    if let Ok(re) = regex::Regex::new(address_pattern) {
        if let Some(caps) = re.captures(text) {
            if let Some(addr) = caps.get(1) {
                entities.push(Entity {
                    kind: EntityKind::Address,
                    value: addr.as_str().to_string(),
                });
            }
        }
    }

    // Extract contact name (to/给 followed by a name)
    let contact_patterns = [
        r"(?:to|给)\s+([A-Za-z][A-Za-z0-9_]{2,15})\b",
        r"(?:给|发给|转给)\s*([^\s\d\.\,，。！？]{2,10})",
    ];

    for pattern in &contact_patterns {
        if let Ok(re) = regex::Regex::new(pattern) {
            if let Some(caps) = re.captures(text) {
                if let Some(contact) = caps.get(1) {
                    entities.push(Entity {
                        kind: EntityKind::Contact,
                        value: contact.as_str().to_string(),
                    });
                    break;
                }
            }
        }
    }

    entities
}

/// Extract token name from price query
fn extract_token(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    let token_pattern = r"\b(ETH|USDC|USDT|BNB|MATIC|ARB|OP|Bitcoin|BTC|Ethereum|以太坊|比特币)\b";
    if let Ok(re) = regex::Regex::new(token_pattern) {
        if let Some(caps) = re.captures(text) {
            if let Some(token) = caps.get(1) {
                let normalized = match token.as_str() {
                    "以太坊" | "Ethereum" => "ETH",
                    "比特币" | "Bitcoin" => "BTC",
                    other => other,
                };
                entities.push(Entity {
                    kind: EntityKind::Token,
                    value: normalized.to_string(),
                });
            }
        }
    }

    entities
}

/// Extract time period from spending summary query
fn extract_time_period(text: &str) -> Vec<Entity> {
    let mut entities = Vec::new();

    let time_patterns = [
        ("today|今天|今日", "today"),
        ("this week|本周|这周", "this_week"),
        ("this month|本月|这个月", "this_month"),
        ("yesterday|昨天", "yesterday"),
        ("last week|上周|上星期", "last_week"),
    ];

    let lower = text.to_lowercase();
    for (pattern, normalized) in &time_patterns {
        if lower.contains(pattern.split('|').next().unwrap()) ||
           pattern.split('|').skip(1).any(|p| text.contains(p))
        {
            entities.push(Entity {
                kind: EntityKind::TimePeriod,
                value: normalized.to_string(),
            });
            break;
        }
    }

    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_balance_intent_zh() {
        let intent = classify("我的余额是多少");
        assert_eq!(intent.kind, IntentKind::CheckBalance);
    }

    #[test]
    fn test_transfer_intent_en() {
        let intent = classify("send 0.1 ETH to alice");
        assert_eq!(intent.kind, IntentKind::Transfer);
        assert!(!intent.entities.is_empty());
    }

    #[test]
    fn test_unknown_intent() {
        let intent = classify("hello world");
        assert_eq!(intent.kind, IntentKind::Unknown);
    }

    #[test]
    fn test_transfer_entity_extraction_full() {
        let intent = classify("send 0.1 ETH to 0x742d35Cc6634C0532925a3b844Bc9e7595f0bEb1");
        assert_eq!(intent.kind, IntentKind::Transfer);

        let amount_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Amount);
        assert!(amount_entity.is_some());
        assert_eq!(amount_entity.unwrap().value, "0.1");

        let token_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Token);
        assert!(token_entity.is_some());
        assert_eq!(token_entity.unwrap().value, "ETH");

        let address_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Address);
        assert!(address_entity.is_some());
    }

    #[test]
    fn test_transfer_entity_extraction_contact() {
        let intent = classify("给 alice 转 100 USDC");
        assert_eq!(intent.kind, IntentKind::Transfer);

        let amount_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Amount);
        assert!(amount_entity.is_some());
        assert_eq!(amount_entity.unwrap().value, "100");

        let token_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Token);
        assert!(token_entity.is_some());
        assert_eq!(token_entity.unwrap().value, "USDC");

        let contact_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Contact);
        assert!(contact_entity.is_some());
        assert_eq!(contact_entity.unwrap().value, "alice");
    }

    #[test]
    fn test_price_query_entity_extraction() {
        let intent = classify("ETH price today");
        assert_eq!(intent.kind, IntentKind::PriceQuery);

        let token_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::Token);
        assert!(token_entity.is_some());
        assert_eq!(token_entity.unwrap().value, "ETH");
    }

    #[test]
    fn test_spending_summary_entity_extraction() {
        let intent = classify("我今天花了多少钱");
        assert_eq!(intent.kind, IntentKind::SpendingSummary);

        let time_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::TimePeriod);
        assert!(time_entity.is_some());
        assert_eq!(time_entity.unwrap().value, "today");
    }

    #[test]
    fn test_spending_summary_this_week() {
        let intent = classify("本周支出统计");
        assert_eq!(intent.kind, IntentKind::SpendingSummary);

        let time_entity = intent.entities.iter()
            .find(|e| e.kind == EntityKind::TimePeriod);
        assert!(time_entity.is_some());
        assert_eq!(time_entity.unwrap().value, "this_week");
    }
}
