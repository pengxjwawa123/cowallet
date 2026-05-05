use crate::services::claude::{ClaudeClient, Message, ContentBlock, ToolDefinition, extract_text, extract_tool_calls};
use crate::services::ai_executor::{ToolContext, ToolExecutionResult};
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

/// Convert internal tool definitions to Claude format
fn wallet_tools_for_claude() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "get_balance".into(),
            description: "Get the user's current wallet balance for a specific token or ETH".into(),
            input_schema: crate::services::claude::ToolSchema {
                schema_type: "object".into(),
                properties: serde_json::json!({
                    "token": {
                        "type": "string",
                        "description": "Token symbol (e.g., ETH, USDC). Omit for native ETH balance."
                    },
                    "chain_id": {
                        "type": "integer",
                        "description": "Chain ID (1 for Ethereum, 8453 for Base, etc.). Default: 8453."
                    }
                }),
                required: vec![],
            },
        },
        ToolDefinition {
            name: "send_transaction".into(),
            description: "Prepare a transaction for sending. Requires user biometric confirmation before execution.".into(),
            input_schema: crate::services::claude::ToolSchema {
                schema_type: "object".into(),
                properties: serde_json::json!({
                    "to_address": {
                        "type": "string",
                        "description": "Recipient wallet address (0x-prefixed hex)"
                    },
                    "value": {
                        "type": "string",
                        "description": "Amount to send in wei (for ETH) or token decimals (for ERC-20)"
                    },
                    "token_address": {
                        "type": "string",
                        "description": "Optional: ERC-20 token contract address. Omit for native ETH."
                    }
                }),
                required: vec!["to_address".into(), "value".into()],
            },
        },
        ToolDefinition {
            name: "get_transaction_history".into(),
            description: "Get recent transaction history for the user's wallet".into(),
            input_schema: crate::services::claude::ToolSchema {
                schema_type: "object".into(),
                properties: serde_json::json!({
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of transactions to return. Default: 20."
                    },
                    "offset": {
                        "type": "integer",
                        "description": "Pagination offset. Default: 0."
                    }
                }),
                required: vec![],
            },
        },
        ToolDefinition {
            name: "search_yield_opportunities".into(),
            description: "Search for DeFi yield opportunities including lending, DEX liquidity pools, liquid staking, and vault strategies. Returns APY data, TVL, risk levels, and smart contract addresses.".into(),
            input_schema: crate::services::claude::ToolSchema {
                schema_type: "object".into(),
                properties: serde_json::json!({
                    "chain_id": {
                        "type": "integer",
                        "description": "Chain ID (8453 for Base, 1 for Ethereum). Default: 8453."
                    },
                    "protocol_type": {
                        "type": "string",
                        "description": "Filter by type: 'dex', 'lending', 'liquid_staking', 'vault', 'farm'"
                    },
                    "min_apy": {
                        "type": "number",
                        "description": "Minimum APY percentage to filter results. Default: 0.0"
                    },
                    "token": {
                        "type": "string",
                        "description": "Filter by token symbol or address (e.g., 'ETH', 'USDC')"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Maximum number of opportunities to return. Default: 20."
                    }
                }),
                required: vec![],
            },
        },
        ToolDefinition {
            name: "list_yield_protocols".into(),
            description: "Get a list of supported DeFi yield protocols with their TVL, risk levels, and audit information.".into(),
            input_schema: crate::services::claude::ToolSchema {
                schema_type: "object".into(),
                properties: serde_json::json!({
                    "chain_id": {
                        "type": "integer",
                        "description": "Filter by chain ID. Omit to return all chains."
                    },
                    "protocol_type": {
                        "type": "string",
                        "description": "Filter by protocol type."
                    }
                }),
                required: vec![],
            },
        },
    ]
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

/// Extended response with tool execution results
#[derive(Debug, Serialize)]
pub struct ChatWithToolsResponse {
    pub message: String,
    pub tool_calls: Vec<ToolCall>,
    pub tool_results: Vec<ToolExecutionResult>,
    pub needs_confirmation: Vec<String>,
}

/// Convert ChatWithToolsResponse to legacy ChatResponse for backward compatibility
impl From<ChatWithToolsResponse> for ChatResponse {
    fn from(ct: ChatWithToolsResponse) -> Self {
        ChatResponse {
            message: ct.message,
            tool_calls: ct.tool_calls,
        }
    }
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

/// Non-streaming chat endpoint with Claude API integration and tool execution
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatWithToolsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_message = req.message.clone();
    let tools = wallet_tools_for_claude();

    // Use Claude API if available, otherwise fall back to rule-based responses
    let claude = match &state.claude {
        Some(c) => c,
        None => {
            tracing::warn!("Claude API not configured, falling back to local classifier");
            return Ok(Json(fallback_response(&user_message)));
        }
    };

    // Convert history and build messages array
    let mut messages: Vec<Message> = req
        .history
        .into_iter()
        .map(|msg| Message {
            role: msg.role,
            content: vec![ContentBlock::Text { text: msg.content }],
        })
        .collect();

    messages.push(Message {
        role: "user".into(),
        content: vec![ContentBlock::Text { text: user_message.clone() }],
    });

    // First call to Claude - get tool calls or direct response
    let response = match claude
        .create_message(&messages, &tools, Some(SYSTEM_PROMPT))
        .await
    {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Claude API failed: {}, falling back to local classifier", e);
            return Ok(Json(fallback_response(&user_message)));
        }
    };

    let initial_text = extract_text(&response);
    let tool_calls = extract_tool_calls(&response);

    // No tools called - return direct response
    if tool_calls.is_empty() {
        return Ok(Json(ChatWithToolsResponse {
            message: initial_text,
            tool_calls: vec![],
            tool_results: vec![],
            needs_confirmation: vec![],
        }));
    }

    // Extract tool calls for response
    let response_tool_calls: Vec<ToolCall> = tool_calls
        .clone()
        .into_iter()
        .map(|(id, name, params)| ToolCall {
            id,
            name,
            parameters: params,
        })
        .collect();

    // Execute tools
    let tool_ctx = ToolContext {
        app_state: state.clone(),
        user_id: req.user_id.clone(),
    };

    let mut tool_results = Vec::new();
    let mut needs_confirmation = Vec::new();

    for (tool_id, name, params) in &tool_calls {
        tracing::info!("Executing tool: {} id={}", name, tool_id);
        let result = tool_ctx.execute_tool(name, tool_id, params.clone()).await;

        // Mark send_transaction as needing confirmation
        if name == "send_transaction" && result.success {
            needs_confirmation.push(tool_id.clone());
        }

        tool_results.push(result);
    }

    // Build tool result content blocks to send back to Claude
    let mut result_blocks = Vec::new();
    for result in &tool_results {
        let content = if result.success {
            serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".into())
        } else {
            format!(
                "Error: {}",
                result.error.as_deref().unwrap_or("unknown error")
            )
        };

        result_blocks.push(ContentBlock::ToolResult {
            tool_use_id: result.tool_id.clone(),
            content,
            is_error: if result.success { None } else { Some(true) },
        });
    }

    // Add assistant response + tool results to conversation history
    messages.push(Message {
        role: "assistant".into(),
        content: response.content.clone(),
    });

    messages.push(Message {
        role: "user".into(),
        content: result_blocks,
    });

    // Second call to Claude - get final response based on tool results
    let final_response = match claude
        .create_message(&messages, &tools, Some(SYSTEM_PROMPT))
        .await
    {
        Ok(r) => extract_text(&r),
        Err(e) => {
            tracing::warn!("Claude API failed after tool execution: {}", e);
            "I retrieved the information but encountered an issue formatting the final response. Please check the tool results below.".into()
        }
    };

    Ok(Json(ChatWithToolsResponse {
        message: final_response,
        tool_calls: response_tool_calls,
        tool_results,
        needs_confirmation,
    }))
}

/// Fallback response using local classifier when Claude is not available
fn fallback_response(user_message: &str) -> ChatWithToolsResponse {
    let classification = classify_intent_locally(user_message);

    let response_message = match classification.intent_type.as_str() {
        "get_balance" => {
            "I can help you check your wallet balance. Enable the Claude API key to enable full functionality."
        }
        "send_transaction" => {
            "Transaction preparation requires the full Claude integration. Please configure your API key to enable this feature."
        }
        "get_transaction_history" => {
            "Transaction history requires database integration. Please configure your database and Claude API key."
        }
        _ => {
            "I'm CoWallet AI assistant. Configure the Claude API key to enable full wallet functionality including balance queries, transaction preparation, and DeFi yield insights."
        }
    };

    ChatWithToolsResponse {
        message: response_message.into(),
        tool_calls: vec![],
        tool_results: vec![],
        needs_confirmation: vec![],
    }
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
