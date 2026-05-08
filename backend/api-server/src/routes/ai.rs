use crate::services::claude::{
    AiClient, Message, ToolDefinition, FunctionDefinition,
    extract_text, extract_tool_calls,
};
use crate::services::ai_executor::{ToolContext, ToolExecutionResult};
use crate::services::chat_store::ChatStore;
use crate::state::AppState;
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{StatusCode, header},
    response::Response,
    routing::{get, post},
};
use bytes::Bytes;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use uuid::Uuid;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat", post(chat_stream))
        .route("/sessions", get(list_sessions).post(create_session))
        .route("/sessions/{session_id}/messages", get(get_session_messages))
        .route("/sessions/{session_id}", axum::routing::delete(delete_session))
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

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
                        "token": { "type": "string", "description": "Token symbol (ETH, USDC, etc.)" },
                        "chain_id": { "type": "integer", "description": "Chain ID. Default: 8453." }
                    },
                    "required": []
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "send_transaction".into(),
                description: "Prepare a transaction. Requires user biometric confirmation.".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "to_address": { "type": "string", "description": "Recipient address (0x-prefixed)" },
                        "value": { "type": "string", "description": "Amount in wei or token decimals" },
                        "token_address": { "type": "string", "description": "Optional: ERC-20 contract address" }
                    },
                    "required": ["to_address", "value"]
                }),
            },
        },
        ToolDefinition {
            tool_type: "function".into(),
            function: FunctionDefinition {
                name: "get_transaction_history".into(),
                description: "Get recent transaction history".into(),
                parameters: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "limit": { "type": "integer", "description": "Max results. Default: 20." },
                        "offset": { "type": "integer", "description": "Pagination offset. Default: 0." }
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
                        "to_address": { "type": "string" },
                        "value": { "type": "string" }
                    },
                    "required": ["to_address", "value"]
                }),
            },
        },
    ]
}

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Request / Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub user_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallInfo {
    pub name: String,
    pub parameters: serde_json::Value,
    pub id: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionQuery {
    pub user_id: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub user_id: String,
    pub title: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SessionInfo {
    pub id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

// ---------------------------------------------------------------------------
// SSE streaming chat — POST /ai/chat
//
// SSE events:
//   event: session     data: {"session_id":"..."}
//   event: token       data: {"text":"..."}
//   event: tool_call   data: {"id":"...","name":"...","parameters":{}}
//   event: tool_result data: {"tool_id":"...","tool_name":"...","success":true,"result":{}}
//   event: done        data: {"needs_confirmation":["..."]}
//   event: error       data: {"message":"..."}
// ---------------------------------------------------------------------------

async fn chat_stream(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Response {
    let user_message = req.message.clone();

    let user_uuid = req.user_id.as_deref()
        .and_then(|id| Uuid::parse_str(id).ok())
        .unwrap_or_else(Uuid::nil);

    let session_id = req.session_id.as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());

    // Resolve session
    let db_session_id = if let Some(db) = &state.db {
        if let Some(sid) = session_id {
            sid
        } else {
            ChatStore::get_or_create_session(db, user_uuid).await
                .map(|s| s.id)
                .unwrap_or_else(|_| Uuid::new_v4())
        }
    } else {
        Uuid::new_v4()
    };

    // Persist user message
    if let Some(db) = &state.db {
        let _ = ChatStore::save_message(db, db_session_id, "user", Some(&user_message), None, None).await;
    }

    // Build the SSE response as a stream
    let stream = async_stream::stream! {
        // Send session_id first
        yield sse_event("session", &serde_json::json!({"session_id": db_session_id.to_string()}));

        let ai = match &state.claude {
            Some(c) => c.clone(),
            None => {
                yield sse_event("error", &serde_json::json!({"message": "AI 服务未配置"}));
                yield sse_event("done", &serde_json::json!({"needs_confirmation": []}));
                return;
            }
        };

        // Build context messages
        let mut messages: Vec<Message> = vec![
            Message { role: "system".into(), content: Some(SYSTEM_PROMPT.into()), tool_calls: None, tool_call_id: None },
        ];

        // Load history from DB
        if let Some(db) = &state.db {
            if let Ok(rows) = ChatStore::load_messages(db, db_session_id, 20).await {
                for row in rows {
                    if row.role == "user" && row.content.as_deref() == Some(user_message.as_str()) {
                        continue;
                    }
                    messages.push(Message {
                        role: row.role,
                        content: row.content,
                        tool_calls: None,
                        tool_call_id: row.tool_call_id,
                    });
                }
            }
        }

        messages.push(Message {
            role: "user".into(),
            content: Some(user_message.clone()),
            tool_calls: None,
            tool_call_id: None,
        });

        let tools = wallet_tools();

        // Stream first response from DeepSeek
        let stream_resp = ai.stream_chat(&messages, &tools, None).await;
        let raw_response = match stream_resp {
            Ok(resp) => resp,
            Err(e) => {
                tracing::error!("AI stream failed: {}", e);
                yield sse_event("error", &serde_json::json!({"message": format!("AI 请求失败: {}", e)}));
                yield sse_event("done", &serde_json::json!({"needs_confirmation": []}));
                return;
            }
        };

        // Parse SSE from upstream DeepSeek
        let mut full_content = String::new();
        let mut tool_calls_acc: Vec<AccToolCall> = Vec::new();
        let mut byte_stream = raw_response.bytes_stream();

        let mut buffer = String::new();

        while let Some(chunk) = byte_stream.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(_) => break,
            };
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete SSE lines
            while let Some(pos) = buffer.find("\n\n") {
                let event_block = buffer[..pos].to_string();
                buffer = buffer[pos+2..].to_string();

                for line in event_block.lines() {
                    if !line.starts_with("data: ") { continue; }
                    let data = &line[6..];
                    if data == "[DONE]" { continue; }

                    if let Ok(chunk) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(choices) = chunk.get("choices").and_then(|c| c.as_array()) {
                            for choice in choices {
                                let delta = match choice.get("delta") {
                                    Some(d) => d,
                                    None => continue,
                                };

                                // Text content
                                if let Some(text) = delta.get("content").and_then(|t| t.as_str()) {
                                    if !text.is_empty() {
                                        full_content.push_str(text);
                                        yield sse_event("token", &serde_json::json!({"text": text}));
                                    }
                                }

                                // Tool calls (accumulated across chunks)
                                if let Some(tcs) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                    for tc in tcs {
                                        let idx = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                                        while tool_calls_acc.len() <= idx {
                                            tool_calls_acc.push(AccToolCall::default());
                                        }
                                        if let Some(id) = tc.get("id").and_then(|s| s.as_str()) {
                                            tool_calls_acc[idx].id = id.to_string();
                                        }
                                        if let Some(f) = tc.get("function") {
                                            if let Some(name) = f.get("name").and_then(|s| s.as_str()) {
                                                tool_calls_acc[idx].name = name.to_string();
                                            }
                                            if let Some(args) = f.get("arguments").and_then(|s| s.as_str()) {
                                                tool_calls_acc[idx].arguments.push_str(args);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // If no tool calls, persist and done
        if tool_calls_acc.is_empty() {
            if let Some(db) = &state.db {
                let _ = ChatStore::save_message(db, db_session_id, "assistant", Some(&full_content), None, None).await;
            }
            yield sse_event("done", &serde_json::json!({"needs_confirmation": []}));
            return;
        }

        // Parse and emit tool calls
        let mut parsed_tool_calls: Vec<ToolCallInfo> = Vec::new();
        for tc in &tool_calls_acc {
            let params: serde_json::Value = serde_json::from_str(&tc.arguments).unwrap_or(serde_json::json!({}));
            parsed_tool_calls.push(ToolCallInfo {
                id: tc.id.clone(),
                name: tc.name.clone(),
                parameters: params.clone(),
            });
            yield sse_event("tool_call", &serde_json::json!({
                "id": tc.id,
                "name": tc.name,
                "parameters": params,
            }));
        }

        // Execute tools
        let tool_ctx = ToolContext {
            app_state: state.clone(),
            user_id: req.user_id.clone(),
        };

        let mut tool_results: Vec<ToolExecutionResult> = Vec::new();
        let mut needs_confirmation: Vec<String> = Vec::new();

        for tc in &parsed_tool_calls {
            let result = tool_ctx.execute_tool(&tc.name, &tc.id, tc.parameters.clone()).await;
            if tc.name == "send_transaction" && result.success {
                needs_confirmation.push(tc.id.clone());
            }
            yield sse_event("tool_result", &serde_json::json!({
                "tool_id": result.tool_id,
                "tool_name": result.tool_name,
                "success": result.success,
                "result": result.result,
                "error": result.error,
            }));
            tool_results.push(result);
        }

        // Build second round messages with tool results
        // Add assistant message with tool_calls
        let tc_for_msg: Vec<crate::services::claude::ToolCall> = tool_calls_acc.iter().map(|tc| {
            crate::services::claude::ToolCall {
                id: tc.id.clone(),
                call_type: "function".into(),
                function: crate::services::claude::FunctionCall {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                },
            }
        }).collect();

        messages.push(Message {
            role: "assistant".into(),
            content: if full_content.is_empty() { None } else { Some(full_content.clone()) },
            tool_calls: Some(tc_for_msg),
            tool_call_id: None,
        });

        for result in &tool_results {
            let content = if result.success {
                serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".into())
            } else {
                format!("Error: {}", result.error.as_deref().unwrap_or("unknown"))
            };
            messages.push(Message {
                role: "tool".into(),
                content: Some(content),
                tool_calls: None,
                tool_call_id: Some(result.tool_id.clone()),
            });
        }

        // Stream second response (after tool results)
        let stream_resp2 = ai.stream_chat(&messages, &tools, None).await;
        match stream_resp2 {
            Ok(resp2) => {
                let mut byte_stream2 = resp2.bytes_stream();
                let mut buffer2 = String::new();
                let mut final_content = String::new();

                while let Some(chunk) = byte_stream2.next().await {
                    let chunk = match chunk {
                        Ok(c) => c,
                        Err(_) => break,
                    };
                    let text = String::from_utf8_lossy(&chunk);
                    buffer2.push_str(&text);

                    while let Some(pos) = buffer2.find("\n\n") {
                        let event_block = buffer2[..pos].to_string();
                        buffer2 = buffer2[pos+2..].to_string();

                        for line in event_block.lines() {
                            if !line.starts_with("data: ") { continue; }
                            let data = &line[6..];
                            if data == "[DONE]" { continue; }

                            if let Ok(chunk_val) = serde_json::from_str::<serde_json::Value>(data) {
                                if let Some(choices) = chunk_val.get("choices").and_then(|c| c.as_array()) {
                                    for choice in choices {
                                        if let Some(text) = choice.get("delta")
                                            .and_then(|d| d.get("content"))
                                            .and_then(|t| t.as_str())
                                        {
                                            if !text.is_empty() {
                                                final_content.push_str(text);
                                                yield sse_event("token", &serde_json::json!({"text": text}));
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Persist final assistant response
                if let Some(db) = &state.db {
                    let tc_json = serde_json::to_value(&parsed_tool_calls).ok();
                    let _ = ChatStore::save_message(db, db_session_id, "assistant", Some(&final_content), tc_json.as_ref(), None).await;
                }
            }
            Err(e) => {
                tracing::error!("AI second stream failed: {}", e);
                yield sse_event("error", &serde_json::json!({"message": "工具结果处理失败"}));
            }
        }

        yield sse_event("done", &serde_json::json!({"needs_confirmation": needs_confirmation}));
    };

    let body = Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(body)
        .unwrap()
}

// ---------------------------------------------------------------------------
// Session management
// ---------------------------------------------------------------------------

async fn create_session(
    State(state): State<AppState>,
    Json(req): Json<CreateSessionRequest>,
) -> Result<Json<SessionInfo>, (StatusCode, Json<serde_json::Value>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "database unavailable"})))
    })?;

    let user_uuid = Uuid::parse_str(&req.user_id).map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid user_id"})))
    })?;

    let session = ChatStore::create_session(db, user_uuid, req.title.as_deref()).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    Ok(Json(SessionInfo {
        id: session.id.to_string(),
        title: session.title,
        created_at: session.created_at.to_rfc3339(),
        updated_at: session.updated_at.to_rfc3339(),
    }))
}

async fn list_sessions(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<SessionQuery>,
) -> Result<Json<Vec<SessionInfo>>, (StatusCode, Json<serde_json::Value>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "database unavailable"})))
    })?;

    let user_uuid = Uuid::parse_str(&query.user_id).map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid user_id"})))
    })?;

    let sessions = ChatStore::list_sessions(db, user_uuid, 50).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    let result: Vec<SessionInfo> = sessions.into_iter().map(|s| SessionInfo {
        id: s.id.to_string(),
        title: s.title,
        created_at: s.created_at.to_rfc3339(),
        updated_at: s.updated_at.to_rfc3339(),
    }).collect();

    Ok(Json(result))
}

async fn get_session_messages(
    State(state): State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> Result<Json<Vec<serde_json::Value>>, (StatusCode, Json<serde_json::Value>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "database unavailable"})))
    })?;

    let session_uuid = Uuid::parse_str(&session_id).map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid session_id"})))
    })?;

    let messages = ChatStore::load_messages(db, session_uuid, 100).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    let result: Vec<serde_json::Value> = messages.into_iter().map(|m| {
        serde_json::json!({
            "id": m.id.to_string(),
            "role": m.role,
            "content": m.content,
            "tool_calls": m.tool_calls,
            "created_at": m.created_at.to_rfc3339(),
        })
    }).collect();

    Ok(Json(result))
}

async fn delete_session(
    State(state): State<AppState>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    axum::extract::Query(query): axum::extract::Query<SessionQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let db = state.db.as_ref().ok_or_else(|| {
        (StatusCode::SERVICE_UNAVAILABLE, Json(serde_json::json!({"error": "database unavailable"})))
    })?;

    let session_uuid = Uuid::parse_str(&session_id).map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid session_id"})))
    })?;

    let user_uuid = Uuid::parse_str(&query.user_id).map_err(|_| {
        (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "invalid user_id"})))
    })?;

    let deleted = ChatStore::delete_session(db, session_uuid, user_uuid).await.map_err(|e| {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()})))
    })?;

    Ok(Json(serde_json::json!({"deleted": deleted})))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

#[derive(Default, Clone)]
struct AccToolCall {
    id: String,
    name: String,
    arguments: String,
}

fn sse_event(event: &str, data: &serde_json::Value) -> Result<Bytes, Infallible> {
    Ok(Bytes::from(format!("event: {}\ndata: {}\n\n", event, data)))
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_sse_event_format() {
        let data = serde_json::json!({"text": "hello"});
        let result = super::sse_event("token", &data).unwrap();
        let s = std::str::from_utf8(&result).unwrap();
        assert!(s.starts_with("event: token\n"));
        assert!(s.contains("\"text\":\"hello\""));
        assert!(s.ends_with("\n\n"));
    }
}
