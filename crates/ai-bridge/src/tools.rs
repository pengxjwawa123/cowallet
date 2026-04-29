use serde::{Deserialize, Serialize};

/// Claude API tool definitions for wallet operations.
///
/// These tools are passed to Claude in the system prompt so the AI
/// can propose wallet actions. Tool outputs are rendered as intent
/// cards on the device — they NEVER execute directly.

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

pub fn wallet_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_balance".into(),
            description: "Get the user's current balance for all tokens or a specific token".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token": {
                        "type": "string",
                        "description": "Token symbol (e.g., ETH, USDC). Omit for all."
                    },
                    "chain": {
                        "type": "string",
                        "description": "Chain name (ethereum, base, arbitrum, optimism, bsc). Omit for all."
                    }
                }
            }),
        },
        ToolDefinition {
            name: "prepare_transfer".into(),
            description: "Prepare a transfer transaction. Returns a confirmation card for user approval. DOES NOT execute.".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "to": { "type": "string", "description": "Recipient address or contact name" },
                    "amount": { "type": "string", "description": "Amount to send" },
                    "token": { "type": "string", "description": "Token symbol (ETH, USDC, etc.)" },
                    "chain": { "type": "string", "description": "Target chain" }
                },
                "required": ["to", "amount", "token"]
            }),
        },
        ToolDefinition {
            name: "get_spending_summary".into(),
            description: "Get categorized spending summary for a time period".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "period": {
                        "type": "string",
                        "enum": ["today", "week", "month"],
                        "description": "Time period for the summary"
                    }
                }
            }),
        },
        ToolDefinition {
            name: "get_price".into(),
            description: "Get current market price of a token".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token": { "type": "string", "description": "Token symbol" }
                },
                "required": ["token"]
            }),
        },
        ToolDefinition {
            name: "explain_contract".into(),
            description: "Analyze and explain a smart contract interaction in plain language".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "address": { "type": "string", "description": "Contract address" },
                    "calldata": { "type": "string", "description": "Transaction calldata (hex)" },
                    "chain": { "type": "string", "description": "Chain name" }
                },
                "required": ["address"]
            }),
        },
        ToolDefinition {
            name: "get_yield_options".into(),
            description: "Find yield-earning opportunities for idle funds".into(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "token": { "type": "string", "description": "Token to earn yield on" },
                    "amount": { "type": "string", "description": "Amount to invest" },
                    "risk_tolerance": {
                        "type": "string",
                        "enum": ["low", "medium", "high"],
                        "description": "Risk tolerance level"
                    }
                }
            }),
        },
    ]
}
