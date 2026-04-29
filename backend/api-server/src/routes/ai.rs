use axum::{Json, Router, routing::post};
use serde::{Deserialize, Serialize};

use crate::state::AppState;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/chat", post(chat))
        .route("/classify", post(classify_intent))
}

#[derive(Deserialize)]
struct ChatRequest {
    message: String,
    history: Vec<ChatMessage>,
}

#[derive(Serialize, Deserialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatResponse {
    message: String,
    tool_calls: Vec<serde_json::Value>,
}

async fn chat(Json(_body): Json<ChatRequest>) -> Json<ChatResponse> {
    // TODO: Proxy to Claude API with wallet tool definitions
    // 1. Prepend system prompt with wallet context
    // 2. Include tool definitions from ai-bridge::tools
    // 3. Stream response back to client via SSE
    Json(ChatResponse {
        message: "TODO: Claude API integration".into(),
        tool_calls: vec![],
    })
}

async fn classify_intent() -> &'static str {
    // TODO: Local intent classification fallback
    "TODO"
}
