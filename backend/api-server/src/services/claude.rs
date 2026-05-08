//! AI chat client — DeepSeek (OpenAI-compatible API).
//!
//! Env vars:
//! - `DEEPSEEK_API_KEY` — API key for DeepSeek
//! - `DEEPSEEK_BASE_URL` — optional, defaults to https://api.deepseek.com
//! - `DEEPSEEK_MODEL` — optional, defaults to deepseek-chat

use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// AI chat client (DeepSeek, OpenAI-compatible)
#[derive(Clone)]
pub struct AiClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

/// A message in the conversation (OpenAI format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Tool call in assistant response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}

/// Tool definition (OpenAI function calling format)
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionDefinition,
}

/// Function definition for tool
#[derive(Debug, Clone, Serialize)]
pub struct FunctionDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

/// Chat completion request
#[derive(Debug, Serialize)]
struct ChatCompletionRequest<'a> {
    model: &'a str,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

/// Chat completion response
#[derive(Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

/// A choice in the response
#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: usize,
    pub message: ChoiceMessage,
    pub finish_reason: Option<String>,
}

/// Message in a choice
#[derive(Debug, Deserialize)]
pub struct ChoiceMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Token usage statistics
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// SSE streaming chunk
#[derive(Debug, Deserialize)]
pub struct StreamChunk {
    pub id: String,
    pub choices: Vec<StreamChoice>,
}

/// A choice in streaming response
#[derive(Debug, Deserialize)]
pub struct StreamChoice {
    pub index: usize,
    pub delta: StreamDelta,
    pub finish_reason: Option<String>,
}

/// Delta content in streaming
#[derive(Debug, Deserialize)]
pub struct StreamDelta {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<StreamToolCall>>,
}

/// Tool call delta in streaming
#[derive(Debug, Deserialize)]
pub struct StreamToolCall {
    pub index: usize,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub function: Option<StreamFunctionCall>,
}

/// Function call delta in streaming
#[derive(Debug, Deserialize)]
pub struct StreamFunctionCall {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub arguments: Option<String>,
}

impl AiClient {
    pub fn new(api_key: String, base_url: Option<String>, model: Option<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            client,
            api_key,
            base_url: base_url.unwrap_or_else(|| "https://api.deepseek.com".into()),
            model: model.unwrap_or_else(|| "deepseek-chat".into()),
        })
    }

    /// Create from environment variables.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        let api_key = std::env::var("DEEPSEEK_API_KEY")
            .map_err(|_| "DEEPSEEK_API_KEY not set")?;
        let base_url = std::env::var("DEEPSEEK_BASE_URL").ok();
        let model = std::env::var("DEEPSEEK_MODEL").ok();
        tracing::info!("Using DeepSeek AI (base={})", base_url.as_deref().unwrap_or("https://api.deepseek.com"));
        Self::new(api_key, base_url, model)
    }

    /// Send a chat completion request
    pub async fn chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        temperature: Option<f32>,
    ) -> Result<ChatCompletionResponse, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let request = ChatCompletionRequest {
            model: &self.model,
            messages,
            tools: if tools.is_empty() { None } else { Some(tools) },
            temperature: Some(temperature.unwrap_or(0.7)),
            max_tokens: Some(4096),
            stream: None,
        };

        let response = self.client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("DeepSeek API error: {}", error_text).into());
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Stream a chat completion response (SSE)
    pub async fn stream_chat(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        temperature: Option<f32>,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        let request = ChatCompletionRequest {
            model: &self.model,
            messages,
            tools: if tools.is_empty() { None } else { Some(tools) },
            temperature: Some(temperature.unwrap_or(0.7)),
            max_tokens: Some(4096),
            stream: Some(true),
        };

        let response = self.client
            .post(&url)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.api_key))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "text/event-stream")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("DeepSeek stream error: {}", error_text).into());
        }

        Ok(response)
    }
}

/// Extract text content from the first choice
pub fn extract_text(response: &ChatCompletionResponse) -> String {
    response
        .choices
        .first()
        .and_then(|c| c.message.content.clone())
        .unwrap_or_default()
}

/// Extract tool calls from the first choice
pub fn extract_tool_calls(response: &ChatCompletionResponse) -> Vec<(String, String, serde_json::Value)> {
    response
        .choices
        .first()
        .and_then(|c| c.message.tool_calls.as_ref())
        .map(|calls| {
            calls.iter().filter_map(|tc| {
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments).ok()?;
                Some((tc.id.clone(), tc.function.name.clone(), args))
            }).collect()
        })
        .unwrap_or_default()
}
