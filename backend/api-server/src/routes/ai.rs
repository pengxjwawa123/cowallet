use crate::services::claude::{AiClient, Message, ToolDefinition, FunctionDefinition, ToolCall as AiToolCall, extract_text, extract_tool_calls};
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

/// Wallet tools in OpenAI function calling format
fn wallet_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_balance".into(),
                description: "Get the user's current wallet balance for a specific token or ETH".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "token": {
                            "type": "string",
                            "description": "Token symbol (e.g., ETH, USDC). Omit for native ETH balance."
                        },
                        "chain_id": {
                            "type": "integer",
                            "description": "Chain ID (1 for Ethereum, 8453 for Base, etc.). Default: 8453."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "send_transaction".into(),
                description: "Prepare a transaction for sending. Requires user biometric confirmation before execution.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
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
                    },
                    "required": ["to_address", "value"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_transaction_history".into(),
                description: "Get recent transaction history for the user's wallet".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "limit": {
                            "type": "integer",
                            "description": "Maximum number of transactions to return. Default: 20."
                        },
                        "offset": {
                            "type": "integer",
                            "description": "Pagination offset. Default: 0."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_wallet_address".into(),
                description: "Get the user's wallet public address".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {},
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "estimate_gas".into(),
                description: "Estimate gas cost for a transaction".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "to_address": {
                            "type": "string",
                            "description": "Recipient wallet address"
                        },
                        "value": {
                            "type": "string",
                            "description": "Amount in wei"
                        }
                    },
                    "required": ["to_address", "value"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "search_yield_opportunities".into(),
                description: "Search for DeFi yield opportunities including lending, DEX liquidity pools, liquid staking, and vault strategies.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
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
                            "description": "Minimum APY percentage. Default: 0.0"
                        },
                        "token": {
                            "type": "string",
                            "description": "Filter by token symbol (e.g., 'ETH', 'USDC')"
                        },
                        "limit": {
                            "type": "integer",
                            "description": "Maximum results to return. Default: 20."
                        }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "list_yield_protocols".into(),
                description: "Get a list of supported DeFi yield protocols with their TVL and risk levels.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "chain_id": {
                            "type": "integer",
                            "description": "Filter by chain ID. Omit to return all chains."
                        },
                        "protocol_type": {
                            "type": "string",
                            "description": "Filter by protocol type."
                        }
                    },
                    "required": []
                }),
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

/// Extended response with tool execution results
#[derive(Debug, Serialize)]
pub struct ChatWithToolsResponse {
    pub message: String,
    pub tool_calls: Vec<ToolCallInfo>,
    pub tool_results: Vec<ToolExecutionResult>,
    pub needs_confirmation: Vec<String>,
}

/// A tool call requested by the AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
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

/// System prompt defining the AI assistant behavior
const SYSTEM_PROMPT: &str = r#"你是 CoWallet，一个 AI 驱动的 MPC 加密货币钱包助手。

核心原则：
1. 安全第一：绝不暴露私钥、助记词或敏感 MPC 数据
2. 确认后操作：任何交易都需要用户明确确认
3. 透明解释：用简单的语言解释你在做什么
4. 中文优先，也支持英文

功能：
- 查询钱包余额（ETH 和代币）
- 估算交易 Gas 费用
- 准备交易（需要用户确认才签名）
- 查看交易历史
- 展示收款地址
- 搜索 DeFi 收益机会

回复风格：
- 简洁友好，专业但不生硬
- 用清晰语言，避免过度使用术语
- 用户想转账时，使用 send_transaction 工具
- 询问余额时，使用 get_balance 工具
- 询问地址时，使用 get_wallet_address 工具
- 询问交易记录时，使用 get_transaction_history 工具

安全规则：
- 绝不模拟或假装发送交易
- 提醒用户区块链交易不可逆
- 提醒用户仔细核对收款地址"#;

/// Simple local intent classifier (fallback when AI not available)
fn classify_intent_locally(message: &str) -> IntentClassification {
    let msg_lower = message.to_lowercase();

    if msg_lower.contains("gas") || msg_lower.contains("fee") || msg_lower.contains("cost")
        || msg_lower.contains("手续费") || msg_lower.contains("gas费")
    {
        return IntentClassification {
            intent_type: "estimate_gas".into(),
            confidence: 0.70,
            entities: serde_json::json!({}),
            suggested_action: Some("Estimate gas cost".into()),
        };
    }

    if msg_lower.contains("balance") || msg_lower.contains("how much") || msg_lower.contains("worth")
        || msg_lower.contains("余额") || msg_lower.contains("多少钱")
    {
        return IntentClassification {
            intent_type: "get_balance".into(),
            confidence: 0.85,
            entities: serde_json::json!({}),
            suggested_action: Some("Check wallet balance".into()),
        };
    }

    if msg_lower.contains("send") || msg_lower.contains("transfer") || msg_lower.contains("pay")
        || msg_lower.contains("转账") || msg_lower.contains("发送") || msg_lower.contains("转")
    {
        return IntentClassification {
            intent_type: "send_transaction".into(),
            confidence: 0.80,
            entities: serde_json::json!({}),
            suggested_action: Some("Prepare transaction".into()),
        };
    }

    if msg_lower.contains("history") || msg_lower.contains("transactions") || msg_lower.contains("recent")
        || msg_lower.contains("记录") || msg_lower.contains("交易")
    {
        return IntentClassification {
            intent_type: "get_transaction_history".into(),
            confidence: 0.75,
            entities: serde_json::json!({}),
            suggested_action: Some("Show transaction history".into()),
        };
    }

    if msg_lower.contains("address") || msg_lower.contains("收款") || msg_lower.contains("地址")
        || msg_lower.contains("qr") || msg_lower.contains("二维码")
    {
        return IntentClassification {
            intent_type: "get_wallet_address".into(),
            confidence: 0.78,
            entities: serde_json::json!({}),
            suggested_action: Some("Show wallet address".into()),
        };
    }

    IntentClassification {
        intent_type: "general_chat".into(),
        confidence: 0.50,
        entities: serde_json::json!({}),
        suggested_action: None,
    }
}

/// Non-streaming chat endpoint with DeepSeek API + tool execution
async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Result<Json<ChatWithToolsResponse>, (StatusCode, Json<serde_json::Value>)> {
    let user_message = req.message.clone();
    let tools = wallet_tools();

    let ai = match &state.claude {
        Some(c) => c,
        None => {
            tracing::warn!("AI client not configured, falling back to local classifier");
            return Ok(Json(fallback_response(&user_message)));
        }
    };

    // Build messages: system + history + user message
    let mut messages: Vec<Message> = vec![
        Message {
            role: "system".into(),
            content: Some(SYSTEM_PROMPT.into()),
            tool_calls: None,
            tool_call_id: None,
        },
    ];

    for msg in &req.history {
        messages.push(Message {
            role: msg.role.clone(),
            content: Some(msg.content.clone()),
            tool_calls: None,
            tool_call_id: None,
        });
    }

    messages.push(Message {
        role: "user".into(),
        content: Some(user_message.clone()),
        tool_calls: None,
        tool_call_id: None,
    });

    // First call — get response or tool calls
    let response = match ai.chat(&messages, &tools, None).await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("AI API failed: {}, falling back to local classifier", e);
            return Ok(Json(fallback_response(&user_message)));
        }
    };

    let initial_text = extract_text(&response);
    let tool_calls = extract_tool_calls(&response);

    // No tools called — return direct response
    if tool_calls.is_empty() {
        return Ok(Json(ChatWithToolsResponse {
            message: initial_text,
            tool_calls: vec![],
            tool_results: vec![],
            needs_confirmation: vec![],
        }));
    }

    // Build response tool calls info
    let response_tool_calls: Vec<ToolCallInfo> = tool_calls
        .iter()
        .map(|(id, name, params)| ToolCallInfo {
            id: id.clone(),
            name: name.clone(),
            parameters: params.clone(),
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

        if name == "send_transaction" && result.success {
            needs_confirmation.push(tool_id.clone());
        }

        tool_results.push(result);
    }

    // Add assistant message with tool_calls to conversation
    let choice = &response.choices[0];
    messages.push(Message {
        role: "assistant".into(),
        content: choice.message.content.clone(),
        tool_calls: choice.message.tool_calls.clone(),
        tool_call_id: None,
    });

    // Add tool results as tool messages
    for result in &tool_results {
        let content = if result.success {
            serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".into())
        } else {
            format!("Error: {}", result.error.as_deref().unwrap_or("unknown error"))
        };

        messages.push(Message {
            role: "tool".into(),
            content: Some(content),
            tool_calls: None,
            tool_call_id: Some(result.tool_id.clone()),
        });
    }

    // Second call — get final response incorporating tool results
    let final_response = match ai.chat(&messages, &tools, None).await {
        Ok(r) => extract_text(&r),
        Err(e) => {
            tracing::warn!("AI API failed after tool execution: {}", e);
            "获取到了信息，但格式化回复时出错。请查看下方的工具执行结果。".into()
        }
    };

    Ok(Json(ChatWithToolsResponse {
        message: final_response,
        tool_calls: response_tool_calls,
        tool_results,
        needs_confirmation,
    }))
}

/// Fallback response using local classifier when AI is not available
fn fallback_response(user_message: &str) -> ChatWithToolsResponse {
    let classification = classify_intent_locally(user_message);

    let response_message = match classification.intent_type.as_str() {
        "get_balance" => "正在查询余额，请稍候...",
        "send_transaction" => "转账功能需要 AI 服务支持，请配置 DEEPSEEK_API_KEY。",
        "get_transaction_history" => "交易记录查询需要 AI 服务支持。",
        "get_wallet_address" => "正在获取钱包地址...",
        _ => "我是 CoWallet AI 助手。配置 DEEPSEEK_API_KEY 后可使用完整的钱包功能。",
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
    let classification = classify_intent_locally(&query.message);

    let stream = async_stream::stream! {
        yield Ok(Event::default().event("thinking").data("Processing..."));

        let response = match classification.intent_type.as_str() {
            "get_balance" => "正在查询钱包余额...",
            "send_transaction" => "准备转账信息...",
            _ => "我是 CoWallet AI 助手，有什么可以帮你的？",
        };

        for word in response.split_whitespace() {
            tokio::time::sleep(Duration::from_millis(50)).await;
            yield Ok(Event::default().event("token").data(word.to_string() + " "));
        }

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
    }

    #[test]
    fn test_classify_balance_zh() {
        let result = classify_intent_locally("我的余额是多少");
        assert_eq!(result.intent_type, "get_balance");
    }

    #[test]
    fn test_classify_send() {
        let result = classify_intent_locally("Send 1 ETH to 0x...");
        assert_eq!(result.intent_type, "send_transaction");
    }

    #[test]
    fn test_classify_send_zh() {
        let result = classify_intent_locally("转账 0.1 ETH");
        assert_eq!(result.intent_type, "send_transaction");
    }

    #[test]
    fn test_classify_address() {
        let result = classify_intent_locally("我的收款地址");
        assert_eq!(result.intent_type, "get_wallet_address");
    }

    #[test]
    fn test_classify_history() {
        let result = classify_intent_locally("最近的交易记录");
        assert_eq!(result.intent_type, "get_transaction_history");
    }

    #[test]
    fn test_classify_general() {
        let result = classify_intent_locally("Hello there");
        assert_eq!(result.intent_type, "general_chat");
    }
}
