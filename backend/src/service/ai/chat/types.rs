use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Per-job stream sender. Each chat job gets its own unbounded channel created
/// at send time, so events produced before the client's WebSocket subscription
/// attaches are buffered (not dropped, as the old shared `broadcast` did). This
/// is what lets us run the agent immediately with no artificial startup delay.
pub type ChatStreamTx = mpsc::UnboundedSender<ChatStreamEnvelope>;

/// Registry of pending job receivers, keyed by job id. The send mutation inserts
/// the receiver here; the `chatStream` subscription removes it when the client
/// attaches and streams from it.
pub type ChatJobRegistry = Arc<Mutex<HashMap<String, mpsc::UnboundedReceiver<ChatStreamEnvelope>>>>;

// --- Stream events sent to frontend via WebSocket ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatStreamEnvelope {
    pub job_id: String,
    pub session_id: String,
    pub kind: ChatStreamKind,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub message_id: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChatStreamKind {
    Token,
    Reasoning,
    ToolStart,
    ToolResult,
    Done,
    Error,
}

impl ChatStreamKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::Reasoning => "reasoning",
            Self::ToolStart => "tool_start",
            Self::ToolResult => "tool_result",
            Self::Done => "done",
            Self::Error => "error",
        }
    }
}

// --- LLM API types (OpenAI-shaped internal wire format; mapped to Gemini in agents/client.rs) ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<LlmToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Multimodal media parts (images/video) attached to this message. Rendered
    /// into Gemini `inline_data` / `file_data` parts by `build_gemini_request`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media: Option<Vec<LlmMediaPart>>,
}

/// A single piece of media attached to an `LlmMessage`. Exactly one of `data`
/// (base64 inline bytes) or `file_uri` (Gemini Files API URI) is expected.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmMediaPart {
    pub mime_type: String,
    /// Base64-encoded inline bytes (used for images).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Gemini Files API file URI (used for video).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_uri: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: LlmFunctionCall,
    /// Gemini thinking models attach an opaque signature to each function call
    /// that must be echoed back when the call is replayed in conversation history.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub thought_signature: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: LlmFunctionDef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LlmFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// --- Chat response from the LLM ---
pub enum LlmChatResponse {
    ToolCall {
        id: String,
        name: String,
        arguments: String,
        thought_signature: Option<String>,
    },
    TextComplete {
        full_text: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserContext {
    pub trade_ids: Option<Vec<String>>,
    pub date_range: Option<DateRange>,
    pub playbook_ids: Option<Vec<String>>,
    pub market_symbol: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}
