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
    ToolStart,
    ToolResult,
    Done,
    Error,
}

impl ChatStreamKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Token => "token",
            Self::ToolStart => "tool_start",
            Self::ToolResult => "tool_result",
            Self::Done => "done",
            Self::Error => "error",
        }
    }
}

// --- Groq API types (OpenAI-compatible) ---
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroqMessage {
    pub role: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<GroqToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroqToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: GroqFunctionCall,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroqFunctionCall {
    pub name: String,
    pub arguments: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroqToolDef {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: GroqFunctionDef,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GroqFunctionDef {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

// --- Chat response from Groq ---
pub enum GroqChatResponse {
    ToolCall {
        id: String,
        name: String,
        arguments: String,
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
