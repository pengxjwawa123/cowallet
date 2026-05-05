//! Anthropic Claude API client for AI-powered wallet assistance.
//!
//! Supports:
//! - Message creation with tool calling
//! - Streaming responses via SSE
//! - Tool result feedback loop

use reqwest::{Client, header};
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Claude API client configuration
#[derive(Clone)]
pub struct ClaudeClient {
    client: Client,
    api_key: String,
    base_url: String,
    model: String,
}

/// A message in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

/// Content block in a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlock {
    #[serde(rename = "text")]
    Text {
        text: String,
    },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

/// Tool definition for Claude
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolSchema,
}

/// JSON Schema for tool inputs
#[derive(Debug, Clone, Serialize)]
pub struct ToolSchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub properties: serde_json::Value,
    pub required: Vec<String>,
}

/// Request to create a message
#[derive(Debug, Serialize)]
struct CreateMessageRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: &'a [Message],
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<&'a [ToolDefinition]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
}

/// Response from create message API
#[derive(Debug, Deserialize)]
pub struct CreateMessageResponse {
    pub id: String,
    pub model: String,
    pub role: String,
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub stop_sequence: Option<String>,
    pub usage: Usage,
}

/// Token usage statistics
#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

/// SSE streaming event
#[derive(Debug, Deserialize)]
pub struct StreamEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub index: Option<usize>,
    #[serde(default)]
    pub content_block: Option<ContentBlockDelta>,
    #[serde(default)]
    pub delta: Option<TextDelta>,
    #[serde(default)]
    pub message: Option<CreateMessageResponse>,
}

/// Delta for text content during streaming
#[derive(Debug, Deserialize)]
pub struct TextDelta {
    #[serde(default)]
    pub text: Option<String>,
}

/// Content block delta during streaming
#[derive(Debug, Deserialize)]
pub struct ContentBlockDelta {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
    pub name: Option<String>,
    pub input: Option<serde_json::Value>,
}

impl ClaudeClient {
    /// Create a new Claude API client
    pub fn new(api_key: String) -> Result<Self, Box<dyn std::error::Error>> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()?;

        Ok(Self {
            client,
            api_key,
            base_url: "https://api.anthropic.com/v1".into(),
            model: "claude-3-sonnet-20240229".into(),
        })
    }

    /// Create a message with optional tool calling
    pub async fn create_message(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<CreateMessageResponse, Box<dyn std::error::Error>> {
        let request = CreateMessageRequest {
            model: &self.model,
            max_tokens: 1024,
            messages,
            tools: if tools.is_empty() { None } else { Some(tools) },
            system: system_prompt,
            stream: false,
            temperature: Some(0.7),
        };

        let response = self
            .client
            .post(&format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header(header::CONTENT_TYPE, "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Claude API error: {}", error_text).into());
        }

        let result = response.json().await?;
        Ok(result)
    }

    /// Stream a message response via SSE
    /// Returns an async stream of events
    pub async fn stream_message(
        &self,
        messages: &[Message],
        tools: &[ToolDefinition],
        system_prompt: Option<&str>,
    ) -> Result<reqwest::Response, Box<dyn std::error::Error>> {
        let request = CreateMessageRequest {
            model: &self.model,
            max_tokens: 1024,
            messages,
            tools: if tools.is_empty() { None } else { Some(tools) },
            system: system_prompt,
            stream: true,
            temperature: Some(0.7),
        };

        let response = self
            .client
            .post(&format!("{}/messages", self.base_url))
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "text/event-stream")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(format!("Claude API error: {}", error_text).into());
        }

        Ok(response)
    }
}

/// Extract tool calls from a Claude response
pub fn extract_tool_calls(response: &CreateMessageResponse) -> Vec<(String, String, serde_json::Value)> {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::ToolUse { id, name, input } => {
                Some((id.clone(), name.clone(), input.clone()))
            }
            _ => None,
        })
        .collect()
}

/// Extract text content from a Claude response
pub fn extract_text(response: &CreateMessageResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_block_serialization() {
        // Test text block
        let text_block = ContentBlock::Text {
            text: "Hello".into(),
        };
        let json = serde_json::to_string(&text_block).unwrap();
        assert!(json.contains("\"type\":\"text\""));

        // Test tool use block
        let tool_block = ContentBlock::ToolUse {
            id: "tool_1".into(),
            name: "get_balance".into(),
            input: serde_json::json!({"token": "ETH"}),
        };
        let json = serde_json::to_string(&tool_block).unwrap();
        assert!(json.contains("\"type\":\"tool_use\""));
    }
}
