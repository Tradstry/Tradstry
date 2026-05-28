use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

// --- Broadcast channel type alias ---
pub type ChatEventBus = broadcast::Sender<ChatStreamEnvelope>;

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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DateRange {
    pub from: String,
    pub to: String,
}
