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
        return Intent {
            kind: IntentKind::Transfer,
            confidence: 0.7,
            entities: vec![],
            raw_text: text_owned,
        };
    }

    if lower.contains("花了") || lower.contains("支出") || lower.contains("spend") {
        return Intent {
            kind: IntentKind::SpendingSummary,
            confidence: 0.8,
            entities: vec![],
            raw_text: text_owned,
        };
    }

    if lower.contains("价格") || lower.contains("price") || lower.contains("多少钱") {
        return Intent {
            kind: IntentKind::PriceQuery,
            confidence: 0.7,
            entities: vec![],
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
    }

    #[test]
    fn test_unknown_intent() {
        let intent = classify("hello world");
        assert_eq!(intent.kind, IntentKind::Unknown);
    }
}
