use crate::state::AppState;
use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::sse::{Event, Sse},
    routing::{get, post},
};
use futures::stream::Stream;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat", post(chat))
        .route("/chat/stream", get(chat_stream))
        .route("/classify", post(classify_intent))
}

/// A chat message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// Request for chat completion
#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub history: Vec<ChatMessage>,
    #[serde(default)]
    pub user_id: Option<String>,
}

/// Response from chat completion
#[derive(Debug, Serialize)]
pub struct ChatResponse {
    pub message: String,
    pub tool_calls: Vec<ToolCall>,
}

/// A tool call requested by the AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub parameters: serde_json::Value,
    pub id: String,
}

/// Classification result for user intent
#[derive(Debug, Serialize)]
pub struct IntentClassification {
    pub intent_type: String,
    pub confidence: f32,
    pub entities: serde_json::Value,
    pub suggested_action: Option<String>,
}

/// Query parameters for SSE streaming
#[derive(Debug, Deserialize)]
pub struct StreamQuery {
    pub message: String,
    #[serde(default)]
    pub user_id: Option<String>,
}

// --- Tool definitions ---

/// All available wallet tools
const TOOLS: &str = r#"
[
  {
    "name": "get_balance",
    "description": "Get the current wallet balance for a specific token or ETH",
    "parameters": {
      "type": "object",
      "properties": {
        "token_address": {
          "type": "string",
          "description": "Optional: ERC-20 token contract address. If omitted, returns ETH balance"
        },
        "chain_id": {
          "type": "integer",
          "description": "Chain ID (1 for Ethereum, 8453 for Base, etc.)",
          "default": 8453
        }
      },
      "required": []
    }
  },
  {
    "name": "send_transaction",
    "description": "Create and send a transaction. Requires user confirmation before execution.",
    "parameters": {
      "type": "object",
      "properties": {
        "to_address": {
          "type": "string",
          "description": "Recipient wallet address"
        },
        "value": {
          "type": "string",
          "description": "Amount in wei (for ETH) or token decimals"
        },
        "token_address": {
          "type": "string",
          "description": "Optional: ERC-20 token contract address for token transfers"
        },
        "gas_limit": {
          "type": "string",
          "description": "Optional: Gas limit for the transaction"
        }
      },
      "required": ["to_address", "value"]
    }
  },
  {
    "name": "estimate_gas",
    "description": "Estimate gas cost for a transaction",
    "parameters": {
      "type": "object",
      "properties": {
        "to_address": {
          "type": "string",
          "description": "Recipient wallet address"
        },
        "value": {
          "type": "string",
          "description": "Amount in wei"
        },
        "data": {
          "type": "string",
          "description": "Optional: Transaction data hex"
        }
      },
      "required": ["to_address", "value"]
    }
  },
  {
    "name": "get_transaction_history",
    "description": "Get recent transaction history for the wallet",
    "parameters": {
      "type": "object",
      "properties": {
        "limit": {
          "type": "integer",
          "description": "Maximum number of transactions to return",
          "default": 20
        },
        "offset": {
          "type": "integer",
          "description": "Pagination offset",
          "default": 0
        }
      },
      "required": []
    }
  },
  {
    "name": "get_wallet_address",
    "description": "Get the user's wallet public address",
    "parameters": {
      "type": "object",
      "properties": {},
      "required": []
    }
  }
]
"#;

/// System prompt defining the AI assistant behavior
const SYSTEM_PROMPT: &str = r#"
You are CoWallet, an AI-powered MPC cryptocurrency wallet assistant.
Your purpose is to help users manage their crypto assets safely and intuitively.

CORE PRINCIPLES:
1. SECURITY FIRST: Never expose private keys, seed phrases, or sensitive MPC data
2. CONFIRM BEFORE ACTION: Always ask for explicit confirmation before any transaction
3. BE TRANSPARENT: Explain what you're doing and why in simple terms
4. HELP USERS UNDERSTAND: Educate users about gas fees, risks, and best practices

CAPABILITIES:
- Check wallet balances (ETH and tokens)
- Estimate gas for transactions
- Prepare transactions (requires user confirmation before signing)
- Show transaction history
- Answer questions about crypto concepts

RESPONSE STYLE:
- Be conversational and friendly but professional
- Use clear language, avoid excessive jargon
- When the user wants to send funds, use the send_transaction tool
- When asked about balance, use the get_balance tool
- When asked about transactions, use the get_transaction_history tool
- For any tool call, clearly explain what you're about to do

SAFETY RULES:
- Never simulate or pretend to send actual transactions
- Always warn about irreversible nature of blockchain transactions
- Remind users to double-check recipient addresses
- If a request seems suspicious, ask for clarification
"#;

/// Simple local intent classifier (fallback when AI not available)
fn classify_intent_locally(message: &str) -> IntentClassification {
    let msg_lower = message.to_lowercase();

    // Gas/fee questions - check before generic "how much"
    if msg_lower.contains("gas") || msg_lower.contains("fee") || msg_lower.contains("cost") {
        return IntentClassification {
            intent_type: "estimate_gas".into(),
            confidence: 0.70,
            entities: serde_json::json!({}),
            suggested_action: Some("Estimate gas cost".into()),
        };
    }

    // Balance queries
    if msg_lower.contains("balance") || msg_lower.contains("how much") || msg_lower.contains("worth") {
        return IntentClassification {
            intent_type: "get_balance".into(),
            confidence: 0.85,
            entities: serde_json::json!({}),
            suggested_action: Some("Check wallet balance".into()),
        };
    }

    // Send/transfer requests
    if msg_lower.contains("send")
        || msg_lower.contains("transfer")
        || msg_lower.contains("pay")
        || msg_lower.contains("give")
    {
        return IntentClassification {
            intent_type: "send_transaction".into(),
            confidence: 0.80,
            entities: serde_json::json!({}),
            suggested_action: Some("Prepare transaction".into()),
        };
    }

    // Transaction history
    if msg_lower.contains("history")
        || msg_lower.contains("transactions")
        || msg_lower.contains("recent")
        || msg_lower.contains("activity")
    {
        return IntentClassification {
            intent_type: "get_transaction_history".into(),
            confidence: 0.75,
            entities: serde_json::json!({}),
            suggested_action: Some("Show transaction history".into()),
        };
    }

    // Address requests
    if msg_lower.contains("address") || msg_lower.contains("my address") || msg_lower.contains("public key") {
        return IntentClassification {
            intent_type: "get_wallet_address".into(),
            confidence: 0.78,
            entities: serde_json::json!({}),
            suggested_action: Some("Show wallet address".into()),
        };
    }

    // Default: general chat
    IntentClassification {
        intent_type: "general_chat".into(),
        confidence: 0.50,
        entities: serde_json::json!({}),
        suggested_action: None,
    }
}

/// Non-streaming chat endpoint
async fn chat(
    State(_state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    // For now, use local intent classification + rule-based responses
    // In production, this would proxy to Claude API with tool calling

    let classification = classify_intent_locally(&req.message);

    let response_message = match classification.intent_type.as_str() {
        "get_balance" => {
            "I can help you check your wallet balance. Let me fetch that for you. This would call the get_balance tool in production."
        }
        "send_transaction" => {
            "I see you want to send a transaction! To help you prepare this, I'll need:\n\n1. The recipient address\n2. The amount to send\n3. Which token (ETH or ERC-20)\n\nPlease note: All transactions require your biometric confirmation before being signed and broadcast."
        }
        "get_transaction_history" => {
            "Let me retrieve your recent transaction history. This would call the get_transaction_history tool in production."
        }
        "estimate_gas" => {
            "I can help estimate gas costs. Gas prices fluctuate based on network demand. Let me check the current conditions for you."
        }
        "get_wallet_address" => {
            "Here's your wallet address. Remember to always double-check before sending funds to it. This would retrieve your actual address in production."
        }
        _ => {
            "I'm CoWallet, your AI-powered MPC crypto wallet assistant! I can help you:\n\n• Check your balance\n• Send transactions (with your confirmation)\n• Review transaction history\n• Estimate gas costs\n• Answer questions about crypto\n\nWhat would you like to do today?"
        }
    };

    // For now, return empty tool calls (demonstration mode)
    // In production: parse Claude response, extract tool calls, execute them, return result
    Json(ChatResponse {
        message: response_message.into(),
        tool_calls: vec![],
    })
}

/// Streaming chat endpoint using SSE
async fn chat_stream(
    State(_state): State<AppState>,
    Query(query): Query<StreamQuery>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Simple streaming demo - in production would stream from Claude API
    let classification = classify_intent_locally(&query.message);

    let stream = async_stream::stream! {
        // Start with thinking event
        yield Ok(Event::default().event("thinking").data("Processing your request..."));

        // Simulate streaming token by token
        let response = match classification.intent_type.as_str() {
            "get_balance" => "Checking your wallet balance now...",
            "send_transaction" => "Let me help you prepare that transaction...",
            _ => "I'm your CoWallet AI assistant. How can I help?",
        };

        // Stream tokens with delay
        for word in response.split_whitespace() {
            tokio::time::sleep(Duration::from_millis(50)).await;
            yield Ok(Event::default().event("token").data(word.to_string() + " "));
        }

        // Final event
        yield Ok(Event::default().event("done").data(""));
    };

    Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive-text"),
    )
}

/// Intent classification endpoint
async fn classify_intent(Json(req): Json<ChatRequest>) -> Json<IntentClassification> {
    Json(classify_intent_locally(&req.message))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_balance() {
        let result = classify_intent_locally("What's my balance?");
        assert_eq!(result.intent_type, "get_balance");
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_classify_send() {
        let result = classify_intent_locally("Send 1 ETH to 0x...");
        assert_eq!(result.intent_type, "send_transaction");
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_classify_history() {
        let result = classify_intent_locally("Show my recent transactions");
        assert_eq!(result.intent_type, "get_transaction_history");
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_classify_gas() {
        let result = classify_intent_locally("How much is gas right now?");
        assert_eq!(result.intent_type, "estimate_gas");
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_classify_address() {
        let result = classify_intent_locally("What's my wallet address?");
        assert_eq!(result.intent_type, "get_wallet_address");
        assert!(result.confidence > 0.5);
    }

    #[test]
    fn test_classify_general() {
        let result = classify_intent_locally("Hello there");
        assert_eq!(result.intent_type, "general_chat");
    }
}
