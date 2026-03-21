# Chat Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a persistent chat system with a ReAct agent that answers natural language questions about trading data using DB queries, hybrid semantic search, and analytics tools.

**Architecture:** Async Rust ReAct loop (not LangGraph StateGraph due to sync-only nodes). Streaming via dedicated broadcast channel → WebSocket subscription. Frontend side panel with Zustand UI state + React Query server state.

**Tech Stack:** Rust (actix-web, async-graphql, reqwest SSE, qdrant-client), TypeScript (Next.js 16, React 19, React Query, Zustand, Shadcn UI)

**Spec:** `docs/superpowers/specs/2026-03-21-chat-agent-design.md`

---

## File Map

### Backend — New Files
| File | Responsibility |
|------|---------------|
| `backend/src/graphql/chat.rs` | ChatQuery, ChatMutation, ChatSubscription resolvers |
| `backend/src/service/chat/mod.rs` | Chat service module (session/message CRUD) |
| `backend/src/service/chat/sessions.rs` | Session create/list/update/delete |
| `backend/src/service/chat/messages.rs` | Message persist/query with cursor pagination |
| `backend/src/service/chat/agent.rs` | ReAct loop (`run_chat_agent`), tool dispatch |
| `backend/src/service/chat/tools/mod.rs` | Tool module |
| `backend/src/service/chat/tools/db_query.rs` | `db_query` tool — structured intent → parameterized SQL |
| `backend/src/service/chat/tools/semantic_search.rs` | `semantic_search` tool — hybrid Qdrant query + rerank |
| `backend/src/service/chat/tools/analytics_calc.rs` | `analytics_calc` tool — compute trading stats |
| `backend/src/service/chat/types.rs` | ChatState, ChatStreamEnvelope, GroqMessage, GroqChatResponse, tool schemas |
| `backend/src/service/agents/vector_database/sparse.rs` | BM25 tokenizer + TF sparse vector generation |

### Backend — Modified Files
| File | Change |
|------|--------|
| `backend/src/service/turso/schema/tables/mod.rs` | Add `chat_sessions` + `chat_messages` tables to SCHEMA_SQL |
| `backend/src/service/turso/schema/logic.rs` | Bump SCHEMA_VERSION |
| `backend/src/service/agents/client.rs` | Add `stream_chat()` method |
| `backend/src/service/agents/vector_database/client.rs` | Add `ensure_hybrid_collection()`, `hybrid_search()`, sparse vector upsert |
| `backend/src/graphql/mod.rs` | Merge ChatQuery, ChatMutation, ChatSubscription |
| `backend/src/service/mod.rs` | Add `pub mod chat;` |
| `backend/src/main.rs` | Create `chat_events_tx` broadcast channel, pass to GraphQL context |
| `backend/src/routes/graphql.rs` | Add ChatEventBus to GraphQL context data |
| `backend/src/service/agents/vector_database/mod.rs` | Add `pub mod sparse;` |

### Frontend — New Files
| File | Responsibility |
|------|---------------|
| `frontend/src/lib/types/chat.ts` | ChatSession, ChatMessage, ChatStreamEvent, ChatContext types |
| `frontend/src/lib/service/chat.ts` | GraphQL queries/mutations/subscriptions for chat |
| `frontend/src/hooks/chat.ts` | useChatSessions, useChatMessages, useSendMessage, useChatStore (Zustand) |
| `frontend/src/components/chat/chat-panel.tsx` | Side panel container |
| `frontend/src/components/chat/chat-session-list.tsx` | Session switcher dropdown |
| `frontend/src/components/chat/chat-message-list.tsx` | Scrollable message area |
| `frontend/src/components/chat/chat-input.tsx` | Input + context picker + send |
| `frontend/src/components/chat/chat-context-picker.tsx` | "+" button popover with tabs |
| `frontend/src/components/chat/chat-stream-message.tsx` | Streaming assistant message renderer |

### Frontend — Modified Files
| File | Change |
|------|--------|
| `frontend/src/components/site-header.tsx` | Wire "Chat AI" button to toggle chat panel |
| `frontend/src/app/dashboard/page.tsx` | Add ChatPanel to layout with conditional split |
| `frontend/package.json` | Add `zustand` dependency |

---

## Task 1: Database Schema — chat_sessions + chat_messages

**Files:**
- Modify: `backend/src/service/turso/schema/tables/mod.rs`
- Modify: `backend/src/service/turso/schema/logic.rs`

- [ ] **Step 1: Add chat tables to SCHEMA_SQL**

In `backend/src/service/turso/schema/tables/mod.rs`, add after the existing table definitions (before the closing of the `SCHEMA_SQL` string):

```sql
CREATE TABLE IF NOT EXISTS chat_sessions (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id),
    account_id TEXT NOT NULL REFERENCES accounts(id),
    title TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE TRIGGER IF NOT EXISTS chat_sessions_updated_at
    AFTER UPDATE ON chat_sessions
    FOR EACH ROW
    BEGIN
        UPDATE chat_sessions SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = OLD.id;
    END;

CREATE TABLE IF NOT EXISTS chat_messages (
    id TEXT PRIMARY KEY,
    session_id TEXT NOT NULL REFERENCES chat_sessions(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK(role IN ('user', 'assistant', 'tool')),
    content TEXT NOT NULL,
    context_json TEXT,
    tool_name TEXT,
    created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_chat_messages_session_created
    ON chat_messages(session_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_chat_sessions_user_account
    ON chat_sessions(user_id, account_id, updated_at DESC);
```

- [ ] **Step 2: Bump schema version**

In `backend/src/service/turso/schema/logic.rs`, increment `SCHEMA_VERSION` by one minor version (e.g., `"1.1"` → `"1.2"`).

- [ ] **Step 3: Build and verify migration**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add backend/src/service/turso/schema/tables/mod.rs backend/src/service/turso/schema/logic.rs
git commit -m "feat(chat): add chat_sessions and chat_messages tables"
```

---

## Task 2: Chat Types + ChatStreamEnvelope

**Files:**
- Create: `backend/src/service/chat/types.rs`
- Create: `backend/src/service/chat/mod.rs`
- Modify: `backend/src/service/mod.rs`

- [ ] **Step 1: Create chat service module**

Create `backend/src/service/chat/mod.rs`:

```rust
pub mod types;
pub mod sessions;
pub mod messages;
pub mod agent;
pub mod tools;
```

- [ ] **Step 2: Create types module**

Create `backend/src/service/chat/types.rs` with:

```rust
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

// --- Groq API types ---
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

// --- Agent state ---
pub struct ChatState {
    pub messages: Vec<GroqMessage>,
    pub user_context: Option<UserContext>,
    pub user_id: String,
    pub account_id: String,
    pub session_id: String,
    pub job_id: String,
    pub iteration_count: u32,
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

pub struct ChatAgentResult {
    pub text: String,
}
```

- [ ] **Step 3: Register module**

In `backend/src/service/mod.rs`, add: `pub mod chat;`

- [ ] **Step 4: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles (some unused warnings OK).

- [ ] **Step 5: Commit**

```bash
git add backend/src/service/chat/ backend/src/service/mod.rs
git commit -m "feat(chat): add chat types and module structure"
```

---

## Task 3: Chat Session + Message CRUD (Service Layer)

**Files:**
- Create: `backend/src/service/chat/sessions.rs`
- Create: `backend/src/service/chat/messages.rs`

- [ ] **Step 1: Create sessions service**

Create `backend/src/service/chat/sessions.rs`:

```rust
use anyhow::Result;
use libsql::Connection;
use uuid::Uuid;

pub struct ChatSession {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

pub async fn create_session(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<ChatSession> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO chat_sessions (id, user_id, account_id) VALUES (?1, ?2, ?3)",
        libsql::params![&id, user_id, account_id],
    ).await?;

    get_session(conn, &id).await
}

pub async fn get_session(conn: &Connection, session_id: &str) -> Result<ChatSession> {
    let mut rows = conn.query(
        "SELECT id, user_id, account_id, title, created_at, updated_at FROM chat_sessions WHERE id = ?1",
        libsql::params![session_id],
    ).await?;

    let row = rows.next().await?.ok_or_else(|| anyhow::anyhow!("Session not found"))?;
    Ok(ChatSession {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        title: row.get::<Option<String>>(3)?,
        created_at: row.get::<String>(4)?,
        updated_at: row.get::<String>(5)?,
    })
}

pub async fn list_sessions(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    limit: i64,
) -> Result<Vec<ChatSession>> {
    let mut rows = conn.query(
        "SELECT id, user_id, account_id, title, created_at, updated_at
         FROM chat_sessions
         WHERE user_id = ?1 AND account_id = ?2
         ORDER BY updated_at DESC
         LIMIT ?3",
        libsql::params![user_id, account_id, limit],
    ).await?;

    let mut sessions = Vec::new();
    while let Some(row) = rows.next().await? {
        sessions.push(ChatSession {
            id: row.get::<String>(0)?,
            user_id: row.get::<String>(1)?,
            account_id: row.get::<String>(2)?,
            title: row.get::<Option<String>>(3)?,
            created_at: row.get::<String>(4)?,
            updated_at: row.get::<String>(5)?,
        });
    }
    Ok(sessions)
}

pub async fn update_session_title(
    conn: &Connection,
    session_id: &str,
    title: &str,
) -> Result<ChatSession> {
    conn.execute(
        "UPDATE chat_sessions SET title = ?1 WHERE id = ?2",
        libsql::params![title, session_id],
    ).await?;
    get_session(conn, session_id).await
}

pub async fn delete_session(
    conn: &Connection,
    session_id: &str,
    user_id: &str,
) -> Result<bool> {
    let rows_affected = conn.execute(
        "DELETE FROM chat_sessions WHERE id = ?1 AND user_id = ?2",
        libsql::params![session_id, user_id],
    ).await?;
    Ok(rows_affected > 0)
}
```

- [ ] **Step 2: Create messages service**

Create `backend/src/service/chat/messages.rs`:

```rust
use anyhow::Result;
use libsql::Connection;
use uuid::Uuid;

pub struct ChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub context_json: Option<String>,
    pub tool_name: Option<String>,
    pub created_at: String,
}

pub async fn insert_message(
    conn: &Connection,
    session_id: &str,
    role: &str,
    content: &str,
    context_json: Option<&str>,
    tool_name: Option<&str>,
) -> Result<ChatMessage> {
    let id = Uuid::new_v4().to_string();
    conn.execute(
        "INSERT INTO chat_messages (id, session_id, role, content, context_json, tool_name)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        libsql::params![&id, session_id, role, content, context_json, tool_name],
    ).await?;

    // Touch session updated_at
    conn.execute(
        "UPDATE chat_sessions SET updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now') WHERE id = ?1",
        libsql::params![session_id],
    ).await?;

    get_message(conn, &id).await
}

pub async fn get_message(conn: &Connection, message_id: &str) -> Result<ChatMessage> {
    let mut rows = conn.query(
        "SELECT id, session_id, role, content, context_json, tool_name, created_at
         FROM chat_messages WHERE id = ?1",
        libsql::params![message_id],
    ).await?;

    let row = rows.next().await?.ok_or_else(|| anyhow::anyhow!("Message not found"))?;
    Ok(ChatMessage {
        id: row.get::<String>(0)?,
        session_id: row.get::<String>(1)?,
        role: row.get::<String>(2)?,
        content: row.get::<String>(3)?,
        context_json: row.get::<Option<String>>(4)?,
        tool_name: row.get::<Option<String>>(5)?,
        created_at: row.get::<String>(6)?,
    })
}

pub async fn list_messages(
    conn: &Connection,
    session_id: &str,
    limit: i64,
    before: Option<&str>,
) -> Result<Vec<ChatMessage>> {
    let (query, params): (String, Vec<libsql::Value>) = if let Some(cursor_id) = before {
        (
            "SELECT m.id, m.session_id, m.role, m.content, m.context_json, m.tool_name, m.created_at
             FROM chat_messages m
             WHERE m.session_id = ?1
               AND m.created_at < (SELECT created_at FROM chat_messages WHERE id = ?2)
             ORDER BY m.created_at DESC
             LIMIT ?3".to_string(),
            vec![session_id.into(), cursor_id.into(), limit.into()],
        )
    } else {
        (
            "SELECT id, session_id, role, content, context_json, tool_name, created_at
             FROM chat_messages
             WHERE session_id = ?1
             ORDER BY created_at DESC
             LIMIT ?2".to_string(),
            vec![session_id.into(), limit.into()],
        )
    };

    let mut rows = conn.query(&query, params).await?;
    let mut messages = Vec::new();
    while let Some(row) = rows.next().await? {
        messages.push(ChatMessage {
            id: row.get::<String>(0)?,
            session_id: row.get::<String>(1)?,
            role: row.get::<String>(2)?,
            content: row.get::<String>(3)?,
            context_json: row.get::<Option<String>>(4)?,
            tool_name: row.get::<Option<String>>(5)?,
            created_at: row.get::<String>(6)?,
        });
    }
    // Reverse so messages are in chronological order
    messages.reverse();
    Ok(messages)
}

/// Load last N user turns with their full tool chains for LLM context
pub async fn load_recent_turns(
    conn: &Connection,
    session_id: &str,
    max_user_turns: i64,
) -> Result<Vec<ChatMessage>> {
    // Get the last N user messages' created_at timestamps
    let mut user_rows = conn.query(
        "SELECT created_at FROM chat_messages
         WHERE session_id = ?1 AND role = 'user'
         ORDER BY created_at DESC
         LIMIT ?2",
        libsql::params![session_id, max_user_turns],
    ).await?;

    let mut cutoff = None;
    let mut count = 0;
    while let Some(row) = user_rows.next().await? {
        cutoff = Some(row.get::<String>(0)?);
        count += 1;
    }

    if count == 0 {
        return Ok(Vec::new());
    }

    // Get all messages from that cutoff point forward
    let cutoff_ts = cutoff.unwrap();
    let mut rows = conn.query(
        "SELECT id, session_id, role, content, context_json, tool_name, created_at
         FROM chat_messages
         WHERE session_id = ?1 AND created_at >= ?2
         ORDER BY created_at ASC",
        libsql::params![session_id, &cutoff_ts],
    ).await?;

    let mut messages = Vec::new();
    while let Some(row) = rows.next().await? {
        messages.push(ChatMessage {
            id: row.get::<String>(0)?,
            session_id: row.get::<String>(1)?,
            role: row.get::<String>(2)?,
            content: row.get::<String>(3)?,
            context_json: row.get::<Option<String>>(4)?,
            tool_name: row.get::<Option<String>>(5)?,
            created_at: row.get::<String>(6)?,
        });
    }
    Ok(messages)
}
```

- [ ] **Step 3: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add backend/src/service/chat/sessions.rs backend/src/service/chat/messages.rs
git commit -m "feat(chat): add session and message CRUD service layer"
```

---

## Task 4: Extend AgentsClient with stream_chat()

**Files:**
- Modify: `backend/src/service/agents/client.rs`

- [ ] **Step 1: Add reqwest + SSE parsing dependencies**

Check `backend/Cargo.toml` — `reqwest` should already be present (used by rig/qdrant). If not, add it with `features = ["stream"]`. Also ensure `futures-util` is available for stream processing.

Run: `cd /Users/user/Tradstry/backend && grep -E "reqwest|futures" Cargo.toml`

- [ ] **Step 2: Add stream_chat() to AgentsClient**

In `backend/src/service/agents/client.rs`, add the following method to the `impl AgentsClient` block. This uses raw reqwest to hit Groq's OpenAI-compatible endpoint with SSE streaming:

```rust
use crate::service::chat::types::*;
use futures_util::StreamExt;

impl AgentsClient {
    // ... existing prompt() method unchanged ...

    pub async fn stream_chat(
        &self,
        messages: &[GroqMessage],
        tools: Option<&[GroqToolDef]>,
        tx: ChatEventBus,
        job_id: &str,
        session_id: &str,
    ) -> Result<GroqChatResponse> {
        let api_key = &self.api_key; // Store api_key in AgentsClient struct
        let model = &self.model;

        let mut body = serde_json::json!({
            "model": model,
            "messages": messages,
            "stream": true,
            "temperature": 0.2,
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::to_value(tools)?;
        }

        let response = reqwest::Client::new()
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            anyhow::bail!("Groq API error {}: {}", status, text);
        }

        let mut stream = response.bytes_stream();
        let mut full_text = String::new();
        let mut tool_call_id = String::new();
        let mut tool_call_name = String::new();
        let mut tool_call_args = String::new();
        let mut has_tool_call = false;
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));

            // Process complete SSE lines
            while let Some(line_end) = buffer.find('\n') {
                let line = buffer[..line_end].trim().to_string();
                buffer = buffer[line_end + 1..].to_string();

                if line.is_empty() || line == "data: [DONE]" {
                    continue;
                }

                if let Some(data) = line.strip_prefix("data: ") {
                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(delta) = parsed["choices"][0]["delta"].as_object() {
                            // Content tokens
                            if let Some(content) = delta.get("content").and_then(|c| c.as_str()) {
                                full_text.push_str(content);
                                let _ = tx.send(ChatStreamEnvelope {
                                    job_id: job_id.to_string(),
                                    session_id: session_id.to_string(),
                                    kind: ChatStreamKind::Token,
                                    content: Some(content.to_string()),
                                    tool_name: None,
                                    message_id: None,
                                });
                            }

                            // Tool calls
                            if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array()) {
                                for tc in tool_calls {
                                    if let Some(func) = tc.get("function") {
                                        if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                                            tool_call_name = name.to_string();
                                            has_tool_call = true;
                                            let _ = tx.send(ChatStreamEnvelope {
                                                job_id: job_id.to_string(),
                                                session_id: session_id.to_string(),
                                                kind: ChatStreamKind::ToolStart,
                                                content: None,
                                                tool_name: Some(name.to_string()),
                                                message_id: None,
                                            });
                                        }
                                        if let Some(args) = func.get("arguments").and_then(|a| a.as_str()) {
                                            tool_call_args.push_str(args);
                                        }
                                    }
                                    if let Some(id) = tc.get("id").and_then(|i| i.as_str()) {
                                        tool_call_id = id.to_string();
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if has_tool_call {
            Ok(GroqChatResponse::ToolCall {
                id: tool_call_id,
                name: tool_call_name,
                arguments: tool_call_args,
            })
        } else {
            Ok(GroqChatResponse::TextComplete {
                full_text,
            })
        }
    }
}
```

Note: The existing `AgentsClient` stores config from env vars. Ensure `api_key` and `model` are accessible as fields (they may currently be consumed by the rig builder — extract them as separate fields during construction).

- [ ] **Step 3: Make api_key and model available as struct fields**

In the `AgentsClient` struct, add `pub api_key: String` and `pub model: String` fields alongside the existing rig client. In `new()`, clone these values before passing to the rig builder.

- [ ] **Step 4: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add backend/src/service/agents/client.rs
git commit -m "feat(chat): add streaming chat with tool calling to AgentsClient"
```

---

## Task 5: BM25 Sparse Vector Generation

**Files:**
- Create: `backend/src/service/agents/vector_database/sparse.rs`
- Modify: `backend/src/service/agents/vector_database/client.rs` (add module import)
- Modify: `backend/src/service/agents/vector_database/mod.rs` (add `pub mod sparse;`)

- [ ] **Step 1: Create sparse vector module**

Create `backend/src/service/agents/vector_database/sparse.rs`:

```rust
use std::collections::HashMap;

/// English stopwords to filter out
const STOPWORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "by", "for", "from",
    "has", "he", "in", "is", "it", "its", "of", "on", "that", "the",
    "to", "was", "were", "will", "with", "the", "this", "but", "they",
    "have", "had", "what", "when", "where", "who", "which", "or", "not",
    "no", "so", "if", "out", "up", "do", "my", "me", "we", "i", "you",
];

/// Tokenize text: lowercase, split on non-alphanumeric, remove stopwords
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .filter(|t| !STOPWORDS.contains(t))
        .map(|t| t.to_string())
        .collect()
}

/// Generate sparse vector from text using term frequency weights.
/// Returns (indices, values) where indices are FNV-hashed token IDs.
pub fn text_to_sparse_vector(text: &str) -> (Vec<u32>, Vec<f32>) {
    let tokens = tokenize(text);
    if tokens.is_empty() {
        return (vec![], vec![]);
    }

    // Count term frequencies
    let mut tf: HashMap<String, u32> = HashMap::new();
    for token in &tokens {
        *tf.entry(token.clone()).or_insert(0) += 1;
    }

    let total = tokens.len() as f32;
    let mut indices: Vec<u32> = Vec::new();
    let mut values: Vec<f32> = Vec::new();

    for (token, count) in &tf {
        let hash = fnv_hash(token);
        let weight = (*count as f32) / total; // normalized TF
        indices.push(hash);
        values.push(weight);
    }

    // Sort by index for consistent ordering
    let mut pairs: Vec<(u32, f32)> = indices.into_iter().zip(values).collect();
    pairs.sort_by_key(|(idx, _)| *idx);
    pairs.into_iter().unzip()
}

/// FNV-1a hash to map tokens to u32 indices
fn fnv_hash(s: &str) -> u32 {
    let mut hash: u32 = 2166136261;
    for byte in s.bytes() {
        hash ^= byte as u32;
        hash = hash.wrapping_mul(16777619);
    }
    hash
}
```

- [ ] **Step 2: Register module**

In `backend/src/service/agents/vector_database/mod.rs` (NOT `client.rs`), add `pub mod sparse;` alongside the existing `pub mod client;`:

```rust
pub mod client;
pub mod sparse;
```

- [ ] **Step 3: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add backend/src/service/agents/vector_database/sparse.rs backend/src/service/agents/vector_database/mod.rs
git commit -m "feat(chat): add BM25 sparse vector generation with FNV hashing"
```

---

## Task 6: Hybrid Search in VectorDatabaseClient

**Files:**
- Modify: `backend/src/service/agents/vector_database/client.rs`

- [ ] **Step 1: Add hybrid collection creation**

Add method `ensure_hybrid_collection()` to `VectorDatabaseClient`:

```rust
pub async fn ensure_hybrid_collection(&self) -> Result<()> {
    use qdrant_client::qdrant::{
        CreateCollectionBuilder, Distance, VectorParamsBuilder,
        SparseVectorParamsBuilder, SparseIndexConfigBuilder,
        vectors_config::Config, VectorsConfig, SparseVectorParams,
    };

    let collection_name = "tradstry_hybrid";

    // Check if exists
    if self.client.collection_exists(collection_name).await? {
        // Ensure indexes exist
        self.ensure_hybrid_indexes(collection_name).await?;
        return Ok(());
    }

    // Create with dense + sparse vectors
    let dense_config = VectorParamsBuilder::new(
        self.embedding_dim as u64,
        Distance::Cosine,
    );

    self.client.create_collection(
        CreateCollectionBuilder::new(collection_name)
            .vectors_config(VectorsConfig {
                config: Some(Config::ParamsMap(/* dense vector named "dense" */)),
            })
            .sparse_vectors_config(/* sparse vector named "sparse" */),
    ).await?;

    self.ensure_hybrid_indexes(collection_name).await?;
    Ok(())
}

async fn ensure_hybrid_indexes(&self, collection_name: &str) -> Result<()> {
    use qdrant_client::qdrant::{FieldType, CreateFieldIndexCollectionBuilder};

    for (field, field_type) in [
        ("user_id", FieldType::Keyword),
        ("account_id", FieldType::Keyword),
        ("source_type", FieldType::Keyword),
        ("created_at", FieldType::Keyword),
    ] {
        let _ = self.client.create_field_index(
            CreateFieldIndexCollectionBuilder::new(collection_name, field, field_type),
        ).await; // Ignore error if index exists
    }
    Ok(())
}
```

Note: The exact Qdrant API for named vectors and sparse vectors varies by qdrant-client version. Consult the qdrant-client 1.17.0 docs for the correct builder API. The intent is: one named dense vector ("dense") + one named sparse vector ("sparse") in the same collection.

- [ ] **Step 2: Add hybrid search method**

```rust
pub async fn hybrid_search(
    &self,
    query_text: &str,
    user_id: &str,
    account_id: &str,
    date_from: Option<&str>,
    date_to: Option<&str>,
    top_k: u64,
) -> Result<Vec<SearchResult>> {
    use crate::service::agents::vector_database::sparse;

    // 1. Generate query vectors
    let dense_vector = self.embed_text(query_text).await?;
    let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(query_text);

    // 2. Build filters
    let mut must_conditions = vec![
        qdrant_client::qdrant::Condition::matches("user_id", user_id.to_string()),
        qdrant_client::qdrant::Condition::matches("account_id", account_id.to_string()),
    ];
    // Add date filters if provided...

    // 3. Query with prefetch (dense + sparse) → RRF fusion
    // Use Qdrant's Query API with prefetch for hybrid search
    // Exact API depends on qdrant-client 1.17.0 query builder

    // 4. Rerank top results
    let results = /* qdrant query results */;
    let texts: Vec<&str> = results.iter().map(|r| r.text.as_str()).collect();
    let reranked = self.rerank(query_text, &texts, top_k as usize).await?;

    Ok(reranked)
}
```

Note: The exact qdrant-client API for prefetch-based hybrid queries should be confirmed against the 1.17.0 docs. The pattern is: two prefetch queries (dense + sparse), fused with RRF, returning top_k * 2 candidates, then reranked by Jina to top_k.

- [ ] **Step 3: Add hybrid upsert method**

```rust
pub async fn upsert_hybrid(
    &self,
    point_id: &str,
    text: &str,
    payload: serde_json::Value,
) -> Result<()> {
    use crate::service::agents::vector_database::sparse;

    let dense = self.embed_text(text).await?;
    let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(text);

    // Upsert point with both dense and sparse named vectors + payload
    // to "tradstry_hybrid" collection
    // Exact API depends on qdrant-client 1.17.0

    Ok(())
}
```

- [ ] **Step 4: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add backend/src/service/agents/vector_database/client.rs
git commit -m "feat(chat): add hybrid search with dense + sparse vectors and RRF fusion"
```

---

## Task 7: Agent Tools (db_query, semantic_search, analytics_calc)

**Files:**
- Create: `backend/src/service/chat/tools/mod.rs`
- Create: `backend/src/service/chat/tools/db_query.rs`
- Create: `backend/src/service/chat/tools/semantic_search.rs`
- Create: `backend/src/service/chat/tools/analytics_calc.rs`

- [ ] **Step 1: Create tools module**

Create `backend/src/service/chat/tools/mod.rs`:

```rust
pub mod db_query;
pub mod semantic_search;
pub mod analytics_calc;

use anyhow::Result;
use crate::service::turso::client::TursoClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;

pub async fn execute_tool(
    name: &str,
    arguments: &str,
    user_id: &str,
    account_id: &str,
    turso: &TursoClient,
    qdrant: &VectorDatabaseClient,
) -> Result<String> {
    match name {
        "db_query" => db_query::execute(arguments, user_id, account_id, turso).await,
        "semantic_search" => semantic_search::execute(arguments, user_id, account_id, qdrant).await,
        "analytics_calc" => analytics_calc::execute(arguments, user_id, account_id, turso).await,
        _ => Ok(format!("Unknown tool: {}", name)),
    }
}

/// Tool schemas for Groq function calling (OpenAI-compatible format)
pub fn tool_schemas() -> Vec<crate::service::chat::types::GroqToolDef> {
    vec![
        db_query::schema(),
        semantic_search::schema(),
        analytics_calc::schema(),
    ]
}
```

- [ ] **Step 2: Create db_query tool**

Create `backend/src/service/chat/tools/db_query.rs`:

```rust
use anyhow::Result;
use serde::Deserialize;
use crate::service::turso::client::TursoClient;
use crate::service::chat::types::GroqToolDef;

#[derive(Deserialize)]
struct DbQueryInput {
    entity: String,
    #[serde(default)]
    filters: DbQueryFilters,
    #[serde(default = "default_limit")]
    limit: i64,
}

#[derive(Deserialize, Default)]
struct DbQueryFilters {
    symbol: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
    trade_type: Option<String>,  // "long" | "short"
}

fn default_limit() -> i64 { 20 }

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: crate::service::chat::types::GroqFunctionDef {
            name: "db_query".to_string(),
            description: "Query the trading database for trades, journal entries, or playbook rules. Use this when the user asks about specific trades, their journal, or playbook configurations.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "entity": {
                        "type": "string",
                        "enum": ["trades", "journal", "playbook"],
                        "description": "Which data to query"
                    },
                    "filters": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string", "description": "Ticker symbol e.g. AAPL, TSLA" },
                            "date_from": { "type": "string", "description": "Start date ISO format" },
                            "date_to": { "type": "string", "description": "End date ISO format" },
                            "trade_type": { "type": "string", "enum": ["long", "short"] }
                        }
                    },
                    "limit": { "type": "integer", "description": "Max results (default 20)" }
                },
                "required": ["entity"]
            }),
        },
    }
}

pub async fn execute(
    arguments: &str,
    user_id: &str,
    account_id: &str,
    turso: &TursoClient,
) -> Result<String> {
    let input: DbQueryInput = serde_json::from_str(arguments)?;
    let conn = turso.get_connection()?;

    let limit = input.limit.min(50); // hard cap

    match input.entity.as_str() {
        "trades" => {
            // Actual columns: id, user_id, account_id, reviewed, open_date, close_date,
            // entry_price, exit_price, position_size, symbol, symbol_name, status,
            // total_pl, net_roi, duration, stop_loss, risk_reward, trade_type,
            // mistakes, entry_tactics, edges_spotted, playbook_id, notes
            let mut conditions = vec!["user_id = ?1".to_string(), "account_id = ?2".to_string()];
            let mut params: Vec<libsql::Value> = vec![user_id.into(), account_id.into()];
            let mut idx = 3;

            if let Some(ref symbol) = input.filters.symbol {
                conditions.push(format!("symbol = ?{}", idx));
                params.push(symbol.clone().into());
                idx += 1;
            }
            if let Some(ref date_from) = input.filters.date_from {
                conditions.push(format!("open_date >= ?{}", idx));
                params.push(date_from.clone().into());
                idx += 1;
            }
            if let Some(ref date_to) = input.filters.date_to {
                conditions.push(format!("close_date <= ?{}", idx));
                params.push(date_to.clone().into());
                idx += 1;
            }
            if let Some(ref trade_type) = input.filters.trade_type {
                conditions.push(format!("trade_type = ?{}", idx));
                params.push(trade_type.clone().into());
            }

            let where_clause = conditions.join(" AND ");
            let query = format!(
                "SELECT id, symbol, symbol_name, trade_type, open_date, close_date,
                        entry_price, exit_price, position_size, total_pl, net_roi,
                        risk_reward, status, notes
                 FROM journal_entries
                 WHERE {}
                 ORDER BY open_date DESC
                 LIMIT {}",
                where_clause, limit
            );

            let mut rows = conn.query(&query, params).await?;
            let mut results = Vec::new();
            while let Some(row) = rows.next().await? {
                results.push(serde_json::json!({
                    "id": row.get::<Option<String>>(0)?,
                    "symbol": row.get::<Option<String>>(1)?,
                    "symbol_name": row.get::<Option<String>>(2)?,
                    "trade_type": row.get::<Option<String>>(3)?,
                    "open_date": row.get::<Option<String>>(4)?,
                    "close_date": row.get::<Option<String>>(5)?,
                    "entry_price": row.get::<Option<f64>>(6)?,
                    "exit_price": row.get::<Option<f64>>(7)?,
                    "position_size": row.get::<Option<f64>>(8)?,
                    "total_pl": row.get::<Option<f64>>(9)?,
                    "net_roi": row.get::<Option<f64>>(10)?,
                    "risk_reward": row.get::<Option<f64>>(11)?,
                    "status": row.get::<Option<String>>(12)?,
                    "notes": row.get::<Option<String>>(13)?,
                }));
            }

            Ok(serde_json::to_string_pretty(&results)?)
        }
        "journal" => {
            // journal_entries with notes focus — same table, different column selection
            // Filter by user_id + account_id
            let mut conditions = vec!["user_id = ?1".to_string(), "account_id = ?2".to_string()];
            let mut params: Vec<libsql::Value> = vec![user_id.into(), account_id.into()];
            let mut idx = 3;

            if let Some(ref date_from) = input.filters.date_from {
                conditions.push(format!("open_date >= ?{}", idx));
                params.push(date_from.clone().into());
                idx += 1;
            }
            if let Some(ref date_to) = input.filters.date_to {
                conditions.push(format!("close_date <= ?{}", idx));
                params.push(date_to.clone().into());
            }

            let where_clause = conditions.join(" AND ");
            let query = format!(
                "SELECT id, symbol, open_date, close_date, notes, mistakes, entry_tactics, edges_spotted
                 FROM journal_entries
                 WHERE {}
                 ORDER BY open_date DESC
                 LIMIT {}",
                where_clause, limit
            );
            let mut rows = conn.query(&query, params).await?;
            let mut results = Vec::new();
            while let Some(row) = rows.next().await? {
                results.push(serde_json::json!({
                    "id": row.get::<Option<String>>(0)?,
                    "symbol": row.get::<Option<String>>(1)?,
                    "open_date": row.get::<Option<String>>(2)?,
                    "close_date": row.get::<Option<String>>(3)?,
                    "notes": row.get::<Option<String>>(4)?,
                    "mistakes": row.get::<Option<String>>(5)?,
                    "entry_tactics": row.get::<Option<String>>(6)?,
                    "edges_spotted": row.get::<Option<String>>(7)?,
                }));
            }
            Ok(serde_json::to_string_pretty(&results)?)
        }
        "playbook" => {
            // Playbooks: filter by user_id only (no account_id column)
            // Actual columns: id, user_id, name, edge_name, entry_rules, exit_rules,
            // position_sizing_rules, additional_rules, created_at, updated_at
            let query = format!(
                "SELECT id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules
                 FROM playbooks
                 WHERE user_id = ?1
                 LIMIT {}",
                limit
            );
            let mut rows = conn.query(&query, libsql::params![user_id]).await?;
            let mut results = Vec::new();
            while let Some(row) = rows.next().await? {
                results.push(serde_json::json!({
                    "id": row.get::<Option<String>>(0)?,
                    "name": row.get::<Option<String>>(1)?,
                    "edge_name": row.get::<Option<String>>(2)?,
                    "entry_rules": row.get::<Option<String>>(3)?,
                    "exit_rules": row.get::<Option<String>>(4)?,
                    "position_sizing_rules": row.get::<Option<String>>(5)?,
                    "additional_rules": row.get::<Option<String>>(6)?,
                }));
            }
            Ok(serde_json::to_string_pretty(&results)?)
        }
        _ => Ok(format!("Unknown entity: {}. Use 'trades', 'journal', or 'playbook'.", input.entity)),
    }
}

- [ ] **Step 3: Create semantic_search tool**

Create `backend/src/service/chat/tools/semantic_search.rs`:

```rust
use anyhow::Result;
use serde::Deserialize;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::chat::types::GroqToolDef;

#[derive(Deserialize)]
struct SearchInput {
    query: String,
    date_from: Option<String>,
    date_to: Option<String>,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: crate::service::chat::types::GroqFunctionDef {
            name: "semantic_search".to_string(),
            description: "Search trading notes, journal entries, and playbooks using semantic search. Use this for questions about patterns, themes, or general insights across the user's data.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "The search query in natural language"
                    },
                    "date_from": { "type": "string", "description": "Optional start date filter" },
                    "date_to": { "type": "string", "description": "Optional end date filter" }
                },
                "required": ["query"]
            }),
        },
    }
}

pub async fn execute(
    arguments: &str,
    user_id: &str,
    account_id: &str,
    qdrant: &VectorDatabaseClient,
) -> Result<String> {
    let input: SearchInput = serde_json::from_str(arguments)?;

    let results = qdrant.hybrid_search(
        &input.query,
        user_id,
        account_id,
        input.date_from.as_deref(),
        input.date_to.as_deref(),
        5, // top 5 after reranking
    ).await?;

    if results.is_empty() {
        return Ok("No results found for this search query.".to_string());
    }

    Ok(serde_json::to_string_pretty(&results)?)
}
```

- [ ] **Step 4: Create analytics_calc tool**

Create `backend/src/service/chat/tools/analytics_calc.rs`:

```rust
use anyhow::Result;
use serde::Deserialize;
use crate::service::turso::client::TursoClient;
use crate::service::chat::types::GroqToolDef;

#[derive(Deserialize)]
struct AnalyticsInput {
    metrics: Vec<String>,
    #[serde(default)]
    filters: AnalyticsFilters,
}

#[derive(Deserialize, Default)]
struct AnalyticsFilters {
    symbol: Option<String>,
    date_from: Option<String>,
    date_to: Option<String>,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: crate::service::chat::types::GroqFunctionDef {
            name: "analytics_calc".to_string(),
            description: "Calculate trading performance metrics like win rate, total P&L, average R-multiple, profit factor, streaks, and per-symbol breakdown. Use this when the user asks about their performance statistics.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "metrics": {
                        "type": "array",
                        "items": {
                            "type": "string",
                            "enum": ["win_rate", "total_pnl", "avg_r", "profit_factor", "streak", "per_symbol"]
                        },
                        "description": "Which metrics to compute"
                    },
                    "filters": {
                        "type": "object",
                        "properties": {
                            "symbol": { "type": "string" },
                            "date_from": { "type": "string" },
                            "date_to": { "type": "string" }
                        }
                    }
                },
                "required": ["metrics"]
            }),
        },
    }
}

pub async fn execute(
    arguments: &str,
    user_id: &str,
    account_id: &str,
    turso: &TursoClient,
) -> Result<String> {
    let input: AnalyticsInput = serde_json::from_str(arguments)?;
    let conn = turso.get_connection()?;

    // Build WHERE clause — always scoped by user_id + account_id
    let mut conditions = vec!["user_id = ?1".to_string(), "account_id = ?2".to_string()];
    let mut params: Vec<libsql::Value> = vec![user_id.into(), account_id.into()];
    let mut idx = 3;

    if let Some(ref symbol) = input.filters.symbol {
        conditions.push(format!("symbol = ?{}", idx));
        params.push(symbol.clone().into());
        idx += 1;
    }
    if let Some(ref date_from) = input.filters.date_from {
        conditions.push(format!("open_date >= ?{}", idx));
        params.push(date_from.clone().into());
        idx += 1;
    }
    if let Some(ref date_to) = input.filters.date_to {
        conditions.push(format!("close_date <= ?{}", idx));
        params.push(date_to.clone().into());
    }

    let where_clause = conditions.join(" AND ");

    // Fetch trades for computation
    // Actual columns: total_pl, symbol, risk_reward (no r_multiple column)
    let query = format!(
        "SELECT total_pl, symbol, risk_reward FROM journal_entries WHERE {} ORDER BY open_date",
        where_clause
    );
    let mut rows = conn.query(&query, params).await?;

    let mut trades: Vec<(f64, String, Option<f64>)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let pnl = row.get::<Option<f64>>(0)?.unwrap_or(0.0);
        let symbol = row.get::<Option<String>>(1)?.unwrap_or_default();
        let risk_reward = row.get::<Option<f64>>(2)?;
        trades.push((pnl, symbol, risk_reward));
    }

    if trades.is_empty() {
        return Ok("No trades found matching the filters.".to_string());
    }

    let mut result = serde_json::Map::new();
    let total = trades.len() as f64;
    let wins: Vec<&(f64, String, Option<f64>)> = trades.iter().filter(|(pnl, _, _)| *pnl > 0.0).collect();
    let losses: Vec<&(f64, String, Option<f64>)> = trades.iter().filter(|(pnl, _, _)| *pnl < 0.0).collect();

    for metric in &input.metrics {
        match metric.as_str() {
            "win_rate" => {
                let rate = (wins.len() as f64 / total) * 100.0;
                result.insert("win_rate".into(), serde_json::json!(format!("{:.1}%", rate)));
                result.insert("total_trades".into(), serde_json::json!(trades.len()));
            }
            "total_pnl" => {
                let total_pnl: f64 = trades.iter().map(|(pnl, _, _)| pnl).sum();
                result.insert("total_pnl".into(), serde_json::json!(format!("{:.2}", total_pnl)));
            }
            "avg_r" => {
                // Uses risk_reward column from journal_entries
                let rr_values: Vec<f64> = trades.iter().filter_map(|(_, _, rr)| *rr).collect();
                if !rr_values.is_empty() {
                    let avg: f64 = rr_values.iter().sum::<f64>() / rr_values.len() as f64;
                    result.insert("avg_risk_reward".into(), serde_json::json!(format!("{:.2}", avg)));
                } else {
                    result.insert("avg_risk_reward".into(), serde_json::json!("N/A (no risk/reward data)"));
                }
            }
            "profit_factor" => {
                let gross_profit: f64 = wins.iter().map(|(pnl, _, _)| pnl).sum();
                let gross_loss: f64 = losses.iter().map(|(pnl, _, _)| pnl.abs()).sum();
                let pf = if gross_loss > 0.0 { gross_profit / gross_loss } else { f64::INFINITY };
                result.insert("profit_factor".into(), serde_json::json!(format!("{:.2}", pf)));
            }
            "streak" => {
                let mut max_win = 0u32;
                let mut max_loss = 0u32;
                let mut cur_win = 0u32;
                let mut cur_loss = 0u32;
                for (pnl, _, _) in &trades {
                    if *pnl > 0.0 {
                        cur_win += 1;
                        cur_loss = 0;
                        max_win = max_win.max(cur_win);
                    } else {
                        cur_loss += 1;
                        cur_win = 0;
                        max_loss = max_loss.max(cur_loss);
                    }
                }
                result.insert("max_win_streak".into(), serde_json::json!(max_win));
                result.insert("max_loss_streak".into(), serde_json::json!(max_loss));
            }
            "per_symbol" => {
                let mut by_symbol: std::collections::HashMap<String, (f64, u32, u32)> = std::collections::HashMap::new();
                for (pnl, sym, _) in &trades {
                    let entry = by_symbol.entry(sym.clone()).or_insert((0.0, 0, 0));
                    entry.0 += pnl;
                    entry.1 += 1;
                    if *pnl > 0.0 { entry.2 += 1; }
                }
                let breakdown: Vec<serde_json::Value> = by_symbol.into_iter().map(|(sym, (pnl, total, wins))| {
                    serde_json::json!({
                        "symbol": sym,
                        "pnl": format!("{:.2}", pnl),
                        "trades": total,
                        "win_rate": format!("{:.1}%", (wins as f64 / total as f64) * 100.0),
                    })
                }).collect();
                result.insert("per_symbol".into(), serde_json::json!(breakdown));
            }
            _ => {}
        }
    }

    Ok(serde_json::to_string_pretty(&serde_json::Value::Object(result))?)
}
```

Note: Column names (`pnl`, `symbol`, `r_multiple`, `entry_date`) must match the actual schema. Verify against `journal_entries` table definition.

- [ ] **Step 5: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add backend/src/service/chat/tools/
git commit -m "feat(chat): add db_query, semantic_search, and analytics_calc tools"
```

---

## Task 8: ReAct Agent Loop

**Files:**
- Create: `backend/src/service/chat/agent.rs`

- [ ] **Step 1: Create the agent module**

Create `backend/src/service/chat/agent.rs`:

```rust
use anyhow::Result;
use log::{info, error};
use uuid::Uuid;
use std::sync::Arc;

use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;
use crate::service::turso::client::TursoClient;
use crate::service::chat::types::*;
use crate::service::chat::tools;
use crate::service::chat::messages;

const MAX_ITERATIONS: u32 = 5;
const MAX_USER_TURNS: i64 = 5;

const SYSTEM_PROMPT: &str = r#"You are a trading assistant for Tradstry. You help users analyze their trading performance, find patterns in their trades, and answer questions about their trading journal, playbooks, and statistics.

You have access to three tools:
- db_query: Query specific trades, journal entries, or playbook rules from the database
- semantic_search: Search across all trading data using natural language (good for finding patterns, themes, similar trades)
- analytics_calc: Calculate performance metrics like win rate, P&L, profit factor, streaks, and per-symbol breakdowns

When the user provides context (pinned trades, date ranges, playbooks), use that to scope your queries. Be specific and data-driven in your responses. Format numbers clearly."#;

pub async fn run_chat_agent(
    session_id: String,
    job_id: String,
    user_message: String,
    user_context: Option<UserContext>,
    user_id: String,
    account_id: String,
    agents: Arc<AgentsClient>,
    turso: Arc<TursoClient>,
    qdrant: Arc<VectorDatabaseClient>,
    tx: ChatEventBus,
) -> Result<()> {
    let conn = turso.get_connection()?;

    // 1. Persist user message
    let context_json = user_context.as_ref().map(|c| serde_json::to_string(c).unwrap_or_default());
    messages::insert_message(
        &conn, &session_id, "user", &user_message,
        context_json.as_deref(), None,
    ).await?;

    // 2. Load conversation history (last 5 user turns)
    let history = messages::load_recent_turns(&conn, &session_id, MAX_USER_TURNS).await?;

    // 3. Build messages for Groq
    let mut groq_messages = vec![GroqMessage {
        role: "system".to_string(),
        content: Some(build_system_prompt(&user_context)),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    // Add history (excluding current message which is already in history)
    for msg in &history {
        groq_messages.push(GroqMessage {
            role: msg.role.clone(),
            content: Some(msg.content.clone()),
            tool_calls: None,
            tool_call_id: if msg.role == "tool" { msg.tool_name.clone() } else { None },
            name: msg.tool_name.clone(),
        });
    }

    // 4. ReAct loop
    let tool_schemas = tools::tool_schemas();
    let mut iteration_count: u32 = 0;

    loop {
        let response = agents.stream_chat(
            &groq_messages,
            if iteration_count < MAX_ITERATIONS { Some(&tool_schemas) } else { None },
            tx.clone(),
            &job_id,
            &session_id,
        ).await;

        match response {
            Ok(GroqChatResponse::ToolCall { id, name, arguments }) => {
                if iteration_count >= MAX_ITERATIONS {
                    // Force final answer
                    let _ = agents.stream_chat(
                        &groq_messages, None, tx.clone(), &job_id, &session_id,
                    ).await;
                    break;
                }

                // Send tool_start event (already sent by stream_chat)
                // Execute tool
                info!("Chat agent executing tool: {} (iteration {})", name, iteration_count);
                let tool_result = match tools::execute_tool(
                    &name, &arguments, &user_id, &account_id, &turso, &qdrant,
                ).await {
                    Ok(result) => result,
                    Err(e) => format!("Tool error: {}", e),
                };

                // Send tool_result event
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.clone(),
                    session_id: session_id.clone(),
                    kind: ChatStreamKind::ToolResult,
                    content: Some(tool_result.clone()),
                    tool_name: Some(name.clone()),
                    message_id: None,
                });

                // Add assistant tool_call + tool result to messages
                groq_messages.push(GroqMessage {
                    role: "assistant".to_string(),
                    content: None,
                    tool_calls: Some(vec![GroqToolCall {
                        id: id.clone(),
                        call_type: "function".to_string(),
                        function: GroqFunctionCall {
                            name: name.clone(),
                            arguments: arguments.clone(),
                        },
                    }]),
                    tool_call_id: None,
                    name: None,
                });
                groq_messages.push(GroqMessage {
                    role: "tool".to_string(),
                    content: Some(tool_result),
                    tool_calls: None,
                    tool_call_id: Some(id),
                    name: Some(name),
                });

                iteration_count += 1;
            }
            Ok(GroqChatResponse::TextComplete { full_text }) => {
                // Persist assistant message
                let msg = messages::insert_message(
                    &conn, &session_id, "assistant", &full_text, None, None,
                ).await?;

                // Send done event with message ID
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.clone(),
                    session_id: session_id.clone(),
                    kind: ChatStreamKind::Done,
                    content: None,
                    tool_name: None,
                    message_id: Some(msg.id),
                });
                break;
            }
            Err(e) => {
                error!("Chat agent error: {}", e);
                // Retry once
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                match agents.stream_chat(
                    &groq_messages,
                    if iteration_count < MAX_ITERATIONS { Some(&tool_schemas) } else { None },
                    tx.clone(), &job_id, &session_id,
                ).await {
                    Ok(GroqChatResponse::TextComplete { full_text }) => {
                        let msg = messages::insert_message(
                            &conn, &session_id, "assistant", &full_text, None, None,
                        ).await?;
                        let _ = tx.send(ChatStreamEnvelope {
                            job_id: job_id.clone(),
                            session_id: session_id.clone(),
                            kind: ChatStreamKind::Done,
                            content: None,
                            tool_name: None,
                            message_id: Some(msg.id),
                        });
                        break;
                    }
                    _ => {
                        let _ = tx.send(ChatStreamEnvelope {
                            job_id: job_id.clone(),
                            session_id: session_id.clone(),
                            kind: ChatStreamKind::Error,
                            content: Some("Sorry, I couldn't process that. Please try again.".to_string()),
                            tool_name: None,
                            message_id: None,
                        });
                        break;
                    }
                }
            }
        }
    }

    // 5. Title generation (parallel, fire-and-forget on first message)
    let is_first_message = history.len() <= 1; // only the message we just inserted
    if is_first_message {
        let agents_clone = agents.clone();
        let turso_clone = turso.clone();
        let session_id_clone = session_id.clone();
        let user_msg = user_message.clone();
        tokio::spawn(async move {
            let title = match agents_clone.prompt(
                format!("Generate a short title (5 words max) for this conversation. Just the title, nothing else: {}", user_msg)
            ).await {
                Ok(t) => t.trim().trim_matches('"').to_string(),
                Err(_) => user_msg.chars().take(50).collect(),
            };
            if let Ok(conn) = turso_clone.get_connection() {
                let _ = crate::service::chat::sessions::update_session_title(
                    &conn, &session_id_clone, &title,
                ).await;
            }
        });
    }

    Ok(())
}

fn build_system_prompt(context: &Option<UserContext>) -> String {
    let mut prompt = SYSTEM_PROMPT.to_string();

    if let Some(ctx) = context {
        prompt.push_str("\n\n## User-provided context:\n");
        if let Some(ref trade_ids) = ctx.trade_ids {
            prompt.push_str(&format!("- Pinned trade IDs: {}\n", trade_ids.join(", ")));
        }
        if let Some(ref range) = ctx.date_range {
            prompt.push_str(&format!("- Date range: {} to {}\n", range.from, range.to));
        }
        if let Some(ref playbook_ids) = ctx.playbook_ids {
            prompt.push_str(&format!("- Pinned playbook IDs: {}\n", playbook_ids.join(", ")));
        }
    }

    prompt
}
```

- [ ] **Step 2: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -5`
Expected: Compiles.

- [ ] **Step 3: Commit**

```bash
git add backend/src/service/chat/agent.rs
git commit -m "feat(chat): implement async ReAct agent loop with streaming and tool dispatch"
```

---

## Task 9: GraphQL Resolvers (chat.rs)

**Files:**
- Create: `backend/src/graphql/chat.rs`
- Modify: `backend/src/graphql/mod.rs`
- Modify: `backend/src/main.rs`
- Modify: `backend/src/routes/graphql.rs`

- [ ] **Step 1: Create chat GraphQL module**

Create `backend/src/graphql/chat.rs`:

```rust
use async_graphql::*;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;
use uuid::Uuid;
use std::sync::Arc;

use crate::service::chat::types::*;
use crate::service::chat::{sessions, messages, agent};
use crate::service::turso::client::TursoClient;
use crate::service::agents::client::AgentsClient;
use crate::service::agents::vector_database::client::VectorDatabaseClient;

// --- GraphQL Types ---

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlChatSession {
    pub id: String,
    pub title: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<sessions::ChatSession> for GqlChatSession {
    fn from(s: sessions::ChatSession) -> Self {
        Self { id: s.id, title: s.title, created_at: s.created_at, updated_at: s.updated_at }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlChatMessage {
    pub id: String,
    pub session_id: String,
    pub role: String,
    pub content: String,
    pub context_json: Option<String>,
    pub tool_name: Option<String>,
    pub created_at: String,
}

impl From<messages::ChatMessage> for GqlChatMessage {
    fn from(m: messages::ChatMessage) -> Self {
        Self {
            id: m.id, session_id: m.session_id, role: m.role, content: m.content,
            context_json: m.context_json, tool_name: m.tool_name, created_at: m.created_at,
        }
    }
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct GqlChatStreamEvent {
    pub job_id: String,
    pub session_id: String,
    pub kind: String,
    pub content: Option<String>,
    pub tool_name: Option<String>,
    pub message_id: Option<String>,
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct ChatContextInput {
    pub trade_ids: Option<Vec<String>>,
    pub date_range: Option<DateRangeInput>,
    pub playbook_ids: Option<Vec<String>>,
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct DateRangeInput {
    pub from: String,
    pub to: String,
}

// --- Helper: resolve user from JWT ---
// IMPORTANT: Must match the ai.rs pattern exactly. jwt.sub is the Clerk UUID,
// NOT the internal user_id. Must call ensure_user() to get the internal user.id.
async fn resolve_user(ctx: &Context<'_>) -> Result<(Arc<TursoClient>, String)> {
    let jwt = ctx.data::<clerk_rs::validators::authorizer::ClerkJwt>()?;
    let turso = ctx.data::<Arc<TursoClient>>()?;
    let conn = turso.get_connection()?;

    let full_name = jwt.other.get("full_name").and_then(|v| v.as_str()).unwrap_or("");
    let email = jwt.other.get("email").and_then(|v| v.as_str()).unwrap_or("");

    // ensure_user returns the internal user record (user.id != jwt.sub)
    let user = crate::service::read_service::users::ensure_user(&conn, &jwt.sub, full_name, email).await?;
    Ok((turso.clone(), user.id))
}

// --- Query ---

#[derive(Default)]
pub struct ChatQuery;

#[Object]
impl ChatQuery {
    async fn chat_sessions(
        &self, ctx: &Context<'_>,
        account_id: String,
        limit: Option<i32>,
    ) -> Result<Vec<GqlChatSession>> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let sessions = sessions::list_sessions(
            &conn, &user_id, &account_id, limit.unwrap_or(20) as i64,
        ).await?;
        Ok(sessions.into_iter().map(GqlChatSession::from).collect())
    }

    async fn chat_messages(
        &self, ctx: &Context<'_>,
        session_id: String,
        limit: Option<i32>,
        before: Option<String>,
    ) -> Result<Vec<GqlChatMessage>> {
        let (turso, _user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let msgs = messages::list_messages(
            &conn, &session_id, limit.unwrap_or(50) as i64, before.as_deref(),
        ).await?;
        Ok(msgs.into_iter().map(GqlChatMessage::from).collect())
    }
}

// --- Mutation ---

#[derive(Default)]
pub struct ChatMutation;

#[Object]
impl ChatMutation {
    async fn create_chat_session(
        &self, ctx: &Context<'_>,
        account_id: String,
    ) -> Result<GqlChatSession> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let session = sessions::create_session(&conn, &user_id, &account_id).await?;
        Ok(GqlChatSession::from(session))
    }

    async fn update_chat_session(
        &self, ctx: &Context<'_>,
        session_id: String,
        title: String,
    ) -> Result<GqlChatSession> {
        let (turso, _user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        let session = sessions::update_session_title(&conn, &session_id, &title).await?;
        Ok(GqlChatSession::from(session))
    }

    async fn delete_chat_session(
        &self, ctx: &Context<'_>,
        session_id: String,
    ) -> Result<bool> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let conn = turso.get_connection()?;
        sessions::delete_session(&conn, &session_id, &user_id).await.map_err(|e| e.into())
    }

    async fn send_chat_message(
        &self, ctx: &Context<'_>,
        session_id: String,
        content: String,
        context: Option<ChatContextInput>,
    ) -> Result<String> {
        let (turso, user_id) = resolve_user(ctx).await?;
        let agents = ctx.data::<Arc<AgentsClient>>()?;
        let qdrant = ctx.data::<Arc<VectorDatabaseClient>>()?;
        let chat_tx = ctx.data::<ChatEventBus>()?;

        // Get session to extract account_id
        let conn = turso.get_connection()?;
        let session = sessions::get_session(&conn, &session_id).await?;

        let job_id = Uuid::new_v4().to_string();

        let user_context = context.map(|c| UserContext {
            trade_ids: c.trade_ids,
            date_range: c.date_range.map(|d| DateRange { from: d.from, to: d.to }),
            playbook_ids: c.playbook_ids,
        });

        // Spawn agent as background task
        let agents = agents.clone();
        let turso = turso.clone();
        let qdrant = qdrant.clone();
        let tx = chat_tx.clone();
        let jid = job_id.clone();

        tokio::spawn(async move {
            if let Err(e) = agent::run_chat_agent(
                session.id, jid.clone(), content, user_context,
                session.user_id, session.account_id,
                agents, turso, qdrant, tx.clone(),
            ).await {
                log::error!("Chat agent failed for job {}: {}", jid, e);
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: jid,
                    session_id: session_id.clone(),
                    kind: ChatStreamKind::Error,
                    content: Some(format!("Agent error: {}", e)),
                    tool_name: None,
                    message_id: None,
                });
            }
        });

        Ok(job_id)
    }
}

// --- Subscription ---

#[derive(Default)]
pub struct ChatSubscription;

#[Subscription]
impl ChatSubscription {
    async fn chat_stream(
        &self, ctx: &Context<'_>,
        job_id: String,
    ) -> impl futures_util::Stream<Item = GqlChatStreamEvent> {
        let tx = ctx.data_unchecked::<ChatEventBus>();
        let rx = tx.subscribe();

        BroadcastStream::new(rx)
            .filter_map(move |event| {
                match event {
                    Ok(envelope) if envelope.job_id == job_id => {
                        Some(GqlChatStreamEvent {
                            job_id: envelope.job_id,
                            session_id: envelope.session_id,
                            kind: envelope.kind.as_str().to_string(),
                            content: envelope.content,
                            tool_name: envelope.tool_name,
                            message_id: envelope.message_id,
                        })
                    }
                    _ => None,
                }
            })
    }
}
```

- [ ] **Step 2: Register chat module in GraphQL schema**

In `backend/src/graphql/mod.rs`:
- Add `pub mod chat;`
- Add `ChatQuery` to the merged `Query` struct
- Add `ChatMutation` to the merged `Mutation` struct
- Add `ChatSubscription` to the merged `Subscription` struct

Follow the existing pattern for how `AiQuery`, `AiMutation`, `AiSubscription` are merged.

- [ ] **Step 3: Add ChatEventBus to main.rs**

In `backend/src/main.rs`, after the existing `ai_events_tx` line:

```rust
let (chat_events_tx, _) = broadcast::channel::<crate::service::chat::types::ChatStreamEnvelope>(256);
```

Pass `chat_events_tx.clone()` to the GraphQL context data alongside `ai_events_tx`.

- [ ] **Step 4: Add ChatEventBus to routes/graphql.rs**

In `backend/src/routes/graphql.rs`, in both `graphql_handler` and `graphql_ws_handler`, add `ChatEventBus` to the request data (same pattern as `AiEventBus`):

```rust
request = request.data(chat_events_tx.clone());
```

- [ ] **Step 5: Build**

Run: `cd /Users/user/Tradstry/backend && cargo build 2>&1 | tail -10`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add backend/src/graphql/chat.rs backend/src/graphql/mod.rs backend/src/main.rs backend/src/routes/graphql.rs
git commit -m "feat(chat): add GraphQL queries, mutations, and streaming subscription"
```

---

## Task 10: Frontend Types + Service Layer

**Files:**
- Create: `frontend/src/lib/types/chat.ts`
- Create: `frontend/src/lib/service/chat.ts`

- [ ] **Step 1: Create chat types**

Create `frontend/src/lib/types/chat.ts`:

```typescript
export interface ChatSession {
  id: string;
  title: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ChatMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "tool";
  content: string;
  contextJson: string | null;
  toolName: string | null;
  createdAt: string;
}

export interface ChatStreamEvent {
  jobId: string;
  sessionId: string;
  kind: "token" | "tool_start" | "tool_result" | "done" | "error";
  content: string | null;
  toolName: string | null;
  messageId: string | null;
}

export interface ChatContext {
  tradeIds?: string[];
  dateRange?: { from: string; to: string };
  playbookIds?: string[];
}
```

- [ ] **Step 2: Create chat service**

Create `frontend/src/lib/service/chat.ts`:

```typescript
import type { GraphQLFetcher } from "@/lib/client";
import type { ChatSession, ChatMessage } from "@/lib/types/chat";

// --- Queries ---

const CHAT_SESSIONS_QUERY = `
  query ChatSessions($accountId: String!, $limit: Int) {
    chatSessions(accountId: $accountId, limit: $limit) {
      id title createdAt updatedAt
    }
  }
`;

const CHAT_MESSAGES_QUERY = `
  query ChatMessages($sessionId: String!, $limit: Int, $before: String) {
    chatMessages(sessionId: $sessionId, limit: $limit, before: $before) {
      id sessionId role content contextJson toolName createdAt
    }
  }
`;

// --- Mutations ---

const CREATE_SESSION_MUTATION = `
  mutation CreateChatSession($accountId: String!) {
    createChatSession(accountId: $accountId) {
      id title createdAt updatedAt
    }
  }
`;

const UPDATE_SESSION_MUTATION = `
  mutation UpdateChatSession($sessionId: String!, $title: String!) {
    updateChatSession(sessionId: $sessionId, title: $title) {
      id title createdAt updatedAt
    }
  }
`;

const DELETE_SESSION_MUTATION = `
  mutation DeleteChatSession($sessionId: String!) {
    deleteChatSession(sessionId: $sessionId)
  }
`;

const SEND_MESSAGE_MUTATION = `
  mutation SendChatMessage($sessionId: String!, $content: String!, $context: ChatContextInput) {
    sendChatMessage(sessionId: $sessionId, content: $content, context: $context)
  }
`;

// --- Subscription ---

export const CHAT_STREAM_SUBSCRIPTION = `
  subscription ChatStream($jobId: String!) {
    chatStream(jobId: $jobId) {
      jobId sessionId kind content toolName messageId
    }
  }
`;

// --- Service functions ---

export async function fetchChatSessions(
  fetcher: GraphQLFetcher,
  accountId: string,
  limit?: number,
): Promise<ChatSession[]> {
  const data = await fetcher<{ chatSessions: ChatSession[] }>(
    CHAT_SESSIONS_QUERY,
    { accountId, limit },
  );
  return data.chatSessions;
}

export async function fetchChatMessages(
  fetcher: GraphQLFetcher,
  sessionId: string,
  limit?: number,
  before?: string,
): Promise<ChatMessage[]> {
  const data = await fetcher<{ chatMessages: ChatMessage[] }>(
    CHAT_MESSAGES_QUERY,
    { sessionId, limit, before },
  );
  return data.chatMessages;
}

export async function createChatSession(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<ChatSession> {
  const data = await fetcher<{ createChatSession: ChatSession }>(
    CREATE_SESSION_MUTATION,
    { accountId },
  );
  return data.createChatSession;
}

export async function updateChatSession(
  fetcher: GraphQLFetcher,
  sessionId: string,
  title: string,
): Promise<ChatSession> {
  const data = await fetcher<{ updateChatSession: ChatSession }>(
    UPDATE_SESSION_MUTATION,
    { sessionId, title },
  );
  return data.updateChatSession;
}

export async function deleteChatSession(
  fetcher: GraphQLFetcher,
  sessionId: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteChatSession: boolean }>(
    DELETE_SESSION_MUTATION,
    { sessionId },
  );
  return data.deleteChatSession;
}

export async function sendChatMessage(
  fetcher: GraphQLFetcher,
  sessionId: string,
  content: string,
  context?: { tradeIds?: string[]; dateRange?: { from: string; to: string }; playbookIds?: string[] },
): Promise<string> {
  const data = await fetcher<{ sendChatMessage: string }>(
    SEND_MESSAGE_MUTATION,
    { sessionId, content, context },
  );
  return data.sendChatMessage;
}
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/lib/types/chat.ts frontend/src/lib/service/chat.ts
git commit -m "feat(chat): add frontend types and GraphQL service layer"
```

---

## Task 11: Frontend Hooks + Zustand Store

**Files:**
- Create: `frontend/src/hooks/chat.ts`
- Modify: `frontend/package.json` (add zustand)

- [ ] **Step 1: Install zustand**

Run: `cd /Users/user/Tradstry/frontend && bun add zustand`

- [ ] **Step 2: Create chat hooks + store**

Create `frontend/src/hooks/chat.ts`:

```typescript
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { create } from "zustand";
import { useGraphQL, useGraphQLSubscription } from "@/lib/client/provider";
import * as chatService from "@/lib/service/chat";
import type { ChatStreamEvent, ChatContext, ChatMessage } from "@/lib/types/chat";
import { useCallback, useRef } from "react";

// --- Zustand Store (ephemeral UI state) ---

interface ChatStore {
  isOpen: boolean;
  activeSessionId: string | null;
  pinnedContext: ChatContext;
  streamingMessage: string;
  streamingToolName: string | null;
  isStreaming: boolean;
  setOpen: (open: boolean) => void;
  toggleOpen: () => void;
  setActiveSession: (id: string | null) => void;
  setPinnedContext: (ctx: ChatContext) => void;
  clearPinnedContext: () => void;
  appendStreamToken: (token: string) => void;
  setStreamingTool: (name: string | null) => void;
  startStreaming: () => void;
  stopStreaming: () => void;
  resetStream: () => void;
}

export const useChatStore = create<ChatStore>((set) => ({
  isOpen: false,
  activeSessionId: null,
  pinnedContext: {},
  streamingMessage: "",
  streamingToolName: null,
  isStreaming: false,
  setOpen: (open) => set({ isOpen: open }),
  toggleOpen: () => set((s) => ({ isOpen: !s.isOpen })),
  setActiveSession: (id) => set({ activeSessionId: id }),
  setPinnedContext: (ctx) => set({ pinnedContext: ctx }),
  clearPinnedContext: () => set({ pinnedContext: {} }),
  appendStreamToken: (token) => set((s) => ({ streamingMessage: s.streamingMessage + token })),
  setStreamingTool: (name) => set({ streamingToolName: name }),
  startStreaming: () => set({ isStreaming: true, streamingMessage: "", streamingToolName: null }),
  stopStreaming: () => set({ isStreaming: false }),
  resetStream: () => set({ streamingMessage: "", streamingToolName: null, isStreaming: false }),
}));

// --- React Query Hooks ---

export function useChatSessions(accountId: string | undefined) {
  const fetcher = useGraphQL();
  return useQuery({
    queryKey: ["chatSessions", accountId],
    queryFn: () => chatService.fetchChatSessions(fetcher, accountId!),
    enabled: !!accountId,
  });
}

export function useChatMessages(sessionId: string | null) {
  const fetcher = useGraphQL();
  return useQuery({
    queryKey: ["chatMessages", sessionId],
    queryFn: () => chatService.fetchChatMessages(fetcher, sessionId!),
    enabled: !!sessionId,
  });
}

export function useSendMessage(accountId: string | undefined) {
  const fetcher = useGraphQL();
  const subscriber = useGraphQLSubscription();
  const queryClient = useQueryClient();
  const store = useChatStore();
  const unsubRef = useRef<(() => void) | null>(null);

  const mutation = useMutation({
    mutationFn: async ({
      sessionId,
      content,
      context,
    }: {
      sessionId: string;
      content: string;
      context?: ChatContext;
    }) => {
      return chatService.sendChatMessage(fetcher, sessionId, content, context);
    },
    onSuccess: (jobId, { sessionId }) => {
      store.startStreaming();

      // Subscribe to stream
      unsubRef.current = subscriber(
        chatService.CHAT_STREAM_SUBSCRIPTION,
        { jobId },
        {
          onMessage: (data: { chatStream: ChatStreamEvent }) => {
            const event = data.chatStream;
            switch (event.kind) {
              case "token":
                if (event.content) store.appendStreamToken(event.content);
                break;
              case "tool_start":
                store.setStreamingTool(event.toolName);
                break;
              case "tool_result":
                store.setStreamingTool(null);
                break;
              case "done":
                store.stopStreaming();
                // Invalidate messages to pick up persisted assistant message
                queryClient.invalidateQueries({ queryKey: ["chatMessages", sessionId] });
                queryClient.invalidateQueries({ queryKey: ["chatSessions", accountId] });
                unsubRef.current?.();
                break;
              case "error":
                store.stopStreaming();
                unsubRef.current?.();
                break;
            }
          },
          onError: () => {
            store.stopStreaming();
            unsubRef.current?.();
          },
          onComplete: () => {
            store.stopStreaming();
          },
        },
      );
    },
  });

  return mutation;
}

export function useCreateSession(accountId: string | undefined) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const store = useChatStore();

  return useMutation({
    mutationFn: () => chatService.createChatSession(fetcher, accountId!),
    onSuccess: (session) => {
      store.setActiveSession(session.id);
      queryClient.invalidateQueries({ queryKey: ["chatSessions", accountId] });
    },
  });
}

export function useDeleteSession(accountId: string | undefined) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const store = useChatStore();

  return useMutation({
    mutationFn: (sessionId: string) => chatService.deleteChatSession(fetcher, sessionId),
    onSuccess: () => {
      store.setActiveSession(null);
      queryClient.invalidateQueries({ queryKey: ["chatSessions", accountId] });
    },
  });
}
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/hooks/chat.ts frontend/package.json frontend/bun.lock
git commit -m "feat(chat): add React Query hooks and Zustand store for chat UI state"
```

---

## Task 12: ChatPanel Component

**Files:**
- Create: `frontend/src/components/chat/chat-panel.tsx`
- Create: `frontend/src/components/chat/chat-message-list.tsx`
- Create: `frontend/src/components/chat/chat-input.tsx`
- Create: `frontend/src/components/chat/chat-stream-message.tsx`
- Create: `frontend/src/components/chat/chat-session-list.tsx`

- [ ] **Step 1: Create ChatPanel**

Create `frontend/src/components/chat/chat-panel.tsx`:

```tsx
"use client";

import { useChatStore, useChatSessions, useChatMessages } from "@/hooks/chat";
import { ChatSessionList } from "./chat-session-list";
import { ChatMessageList } from "./chat-message-list";
import { ChatInput } from "./chat-input";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";

interface ChatPanelProps {
  accountId: string;
}

export function ChatPanel({ accountId }: ChatPanelProps) {
  const { isOpen, activeSessionId, setOpen } = useChatStore();

  if (!isOpen) return null;

  return (
    <div className="w-[380px] border-l flex flex-col h-full bg-background">
      {/* Header */}
      <div className="flex items-center justify-between p-3 border-b">
        <ChatSessionList accountId={accountId} />
        <Button variant="ghost" size="icon" onClick={() => setOpen(false)}>
          <X className="h-4 w-4" />
        </Button>
      </div>

      {/* Messages */}
      <div className="flex-1 overflow-hidden">
        {activeSessionId ? (
          <ChatMessageList sessionId={activeSessionId} />
        ) : (
          <div className="flex items-center justify-center h-full text-muted-foreground text-sm">
            Start a new conversation
          </div>
        )}
      </div>

      {/* Input */}
      {activeSessionId && (
        <ChatInput sessionId={activeSessionId} accountId={accountId} />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Create ChatMessageList**

Create `frontend/src/components/chat/chat-message-list.tsx`:

```tsx
"use client";

import { useChatMessages, useChatStore } from "@/hooks/chat";
import { ChatStreamMessage } from "./chat-stream-message";
import { useEffect, useRef } from "react";

interface ChatMessageListProps {
  sessionId: string;
}

export function ChatMessageList({ sessionId }: ChatMessageListProps) {
  const { data: messages, isLoading } = useChatMessages(sessionId);
  const { isStreaming, streamingMessage, streamingToolName } = useChatStore();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingMessage]);

  if (isLoading) {
    return <div className="p-4 text-sm text-muted-foreground">Loading...</div>;
  }

  return (
    <div className="overflow-y-auto h-full p-3 space-y-3">
      {messages?.map((msg) => (
        <div key={msg.id} className={`text-sm ${msg.role === "user" ? "text-right" : ""}`}>
          {msg.role === "user" ? (
            <div className="inline-block bg-primary text-primary-foreground rounded-lg px-3 py-2 max-w-[85%]">
              {msg.content}
            </div>
          ) : msg.role === "assistant" ? (
            <div className="bg-muted rounded-lg px-3 py-2 max-w-[85%]">
              {msg.content}
            </div>
          ) : null}
        </div>
      ))}

      {isStreaming && (
        <ChatStreamMessage
          content={streamingMessage}
          toolName={streamingToolName}
        />
      )}

      <div ref={bottomRef} />
    </div>
  );
}
```

- [ ] **Step 3: Create ChatStreamMessage**

Create `frontend/src/components/chat/chat-stream-message.tsx`:

```tsx
"use client";

interface ChatStreamMessageProps {
  content: string;
  toolName: string | null;
}

export function ChatStreamMessage({ content, toolName }: ChatStreamMessageProps) {
  return (
    <div className="text-sm">
      {toolName && (
        <div className="flex items-center gap-2 text-xs text-muted-foreground mb-1">
          <span className="animate-spin h-3 w-3 border border-current border-t-transparent rounded-full" />
          Searching: {toolName}...
        </div>
      )}
      {content && (
        <div className="bg-muted rounded-lg px-3 py-2 max-w-[85%]">
          {content}
          <span className="animate-pulse">▌</span>
        </div>
      )}
    </div>
  );
}
```

- [ ] **Step 4: Create ChatInput**

Create `frontend/src/components/chat/chat-input.tsx`:

```tsx
"use client";

import { useState } from "react";
import { useSendMessage, useChatStore } from "@/hooks/chat";
import { Button } from "@/components/ui/button";
import { Send, Plus } from "lucide-react";

interface ChatInputProps {
  sessionId: string;
  accountId: string;
}

export function ChatInput({ sessionId, accountId }: ChatInputProps) {
  const [input, setInput] = useState("");
  const { isStreaming, pinnedContext, resetStream } = useChatStore();
  const sendMessage = useSendMessage(accountId);

  const handleSend = () => {
    if (!input.trim() || isStreaming) return;
    resetStream();
    sendMessage.mutate({
      sessionId,
      content: input.trim(),
      context: Object.keys(pinnedContext).length > 0 ? pinnedContext : undefined,
    });
    setInput("");
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  return (
    <div className="border-t p-3">
      {/* Pinned context badges would go here */}
      <div className="flex items-end gap-2">
        <Button variant="ghost" size="icon" className="shrink-0" disabled={isStreaming}>
          <Plus className="h-4 w-4" />
        </Button>
        <textarea
          value={input}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Ask about your trades..."
          className="flex-1 resize-none border rounded-md px-3 py-2 text-sm min-h-[40px] max-h-[120px] bg-background"
          rows={1}
          disabled={isStreaming}
        />
        <Button
          size="icon"
          onClick={handleSend}
          disabled={!input.trim() || isStreaming}
          className="shrink-0"
        >
          <Send className="h-4 w-4" />
        </Button>
      </div>
    </div>
  );
}
```

- [ ] **Step 5: Create ChatSessionList**

Create `frontend/src/components/chat/chat-session-list.tsx`:

```tsx
"use client";

import { useChatSessions, useCreateSession, useChatStore } from "@/hooks/chat";
import { Button } from "@/components/ui/button";
import { Plus } from "lucide-react";

interface ChatSessionListProps {
  accountId: string;
}

export function ChatSessionList({ accountId }: ChatSessionListProps) {
  const { data: sessions } = useChatSessions(accountId);
  const createSession = useCreateSession(accountId);
  const { activeSessionId, setActiveSession } = useChatStore();

  return (
    <div className="flex items-center gap-2">
      <select
        value={activeSessionId || ""}
        onChange={(e) => setActiveSession(e.target.value || null)}
        className="text-sm bg-background border rounded px-2 py-1 max-w-[200px]"
      >
        <option value="">Select chat...</option>
        {sessions?.map((s) => (
          <option key={s.id} value={s.id}>
            {s.title || "New conversation"}
          </option>
        ))}
      </select>
      <Button
        variant="ghost"
        size="icon"
        onClick={() => createSession.mutate()}
        className="h-7 w-7"
      >
        <Plus className="h-3 w-3" />
      </Button>
    </div>
  );
}
```

- [ ] **Step 6: Commit**

```bash
git add frontend/src/components/chat/
git commit -m "feat(chat): add ChatPanel, MessageList, Input, StreamMessage, and SessionList components"
```

---

## Task 13: Wire Chat Panel into Dashboard Layout

**Files:**
- Modify: `frontend/src/app/dashboard/page.tsx`
- Modify: `frontend/src/components/site-header.tsx`

- [ ] **Step 1: Add ChatPanel to dashboard layout**

In `frontend/src/app/dashboard/page.tsx`, import and add the `ChatPanel` alongside the main content area. Wrap the main content + chat panel in a flex container:

```tsx
import { ChatPanel } from "@/components/chat/chat-panel";
import { useChatStore } from "@/hooks/chat";

// Inside the component, after SidebarInset:
<div className="flex flex-1 overflow-hidden">
  <div className="flex-1 overflow-auto">
    {/* existing main content */}
  </div>
  <ChatPanel accountId={accountId} />
</div>
```

The `accountId` should come from the existing account context (check how it's passed in the current dashboard).

- [ ] **Step 2: Wire the Chat AI button**

In `frontend/src/components/site-header.tsx`, import `useChatStore` and wire the button:

```tsx
import { useChatStore } from "@/hooks/chat";

// Inside the component:
const toggleChat = useChatStore((s) => s.toggleOpen);

// On the Chat AI button:
<Button variant="ghost" onClick={toggleChat}>
  {/* existing icon */} Chat AI
</Button>
```

- [ ] **Step 3: Verify in browser**

Run the dev server and verify:
1. Chat AI button toggles the side panel
2. Panel shows session list + create button
3. Creating a session enables the input
4. Main content area shrinks when panel opens

- [ ] **Step 4: Commit**

```bash
git add frontend/src/app/dashboard/page.tsx frontend/src/components/site-header.tsx
git commit -m "feat(chat): wire ChatPanel into dashboard layout with toggle button"
```

---

## Task 14: Context Picker Component

**Files:**
- Create: `frontend/src/components/chat/chat-context-picker.tsx`
- Modify: `frontend/src/components/chat/chat-input.tsx`

- [ ] **Step 1: Create ChatContextPicker**

Create `frontend/src/components/chat/chat-context-picker.tsx`:

```tsx
"use client";

import { useState } from "react";
import { useChatStore } from "@/hooks/chat";
import { Button } from "@/components/ui/button";
import { X } from "lucide-react";

interface ChatContextPickerProps {
  onClose: () => void;
}

export function ChatContextPicker({ onClose }: ChatContextPickerProps) {
  const { pinnedContext, setPinnedContext } = useChatStore();
  const [activeTab, setActiveTab] = useState<"dateRange" | "trades" | "playbooks">("dateRange");
  const [dateFrom, setDateFrom] = useState(pinnedContext.dateRange?.from || "");
  const [dateTo, setDateTo] = useState(pinnedContext.dateRange?.to || "");

  const applyDateRange = () => {
    if (dateFrom && dateTo) {
      setPinnedContext({ ...pinnedContext, dateRange: { from: dateFrom, to: dateTo } });
      onClose();
    }
  };

  return (
    <div className="absolute bottom-full left-0 mb-2 w-72 bg-popover border rounded-lg shadow-lg p-3 z-50">
      <div className="flex justify-between items-center mb-2">
        <span className="text-sm font-medium">Add Context</span>
        <Button variant="ghost" size="icon" className="h-6 w-6" onClick={onClose}>
          <X className="h-3 w-3" />
        </Button>
      </div>

      {/* Tabs */}
      <div className="flex gap-1 mb-3">
        {(["dateRange", "trades", "playbooks"] as const).map((tab) => (
          <button
            key={tab}
            onClick={() => setActiveTab(tab)}
            className={`text-xs px-2 py-1 rounded ${activeTab === tab ? "bg-primary text-primary-foreground" : "bg-muted"}`}
          >
            {tab === "dateRange" ? "Date Range" : tab === "trades" ? "Trades" : "Playbooks"}
          </button>
        ))}
      </div>

      {/* Date Range Tab */}
      {activeTab === "dateRange" && (
        <div className="space-y-2">
          <input type="date" value={dateFrom} onChange={(e) => setDateFrom(e.target.value)}
            className="w-full border rounded px-2 py-1 text-sm bg-background" />
          <input type="date" value={dateTo} onChange={(e) => setDateTo(e.target.value)}
            className="w-full border rounded px-2 py-1 text-sm bg-background" />
          <Button size="sm" onClick={applyDateRange} className="w-full">Apply</Button>
        </div>
      )}

      {/* Trades + Playbooks tabs: placeholder — requires fetching trade/playbook lists */}
      {activeTab === "trades" && (
        <div className="text-xs text-muted-foreground">Trade selection coming soon</div>
      )}
      {activeTab === "playbooks" && (
        <div className="text-xs text-muted-foreground">Playbook selection coming soon</div>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Wire context picker into ChatInput**

In `frontend/src/components/chat/chat-input.tsx`, add state for the picker popover and render `ChatContextPicker` when the "+" button is clicked. Also render pinned context badges above the input.

```tsx
const [showPicker, setShowPicker] = useState(false);

// In the "+" button:
<Button variant="ghost" size="icon" onClick={() => setShowPicker(!showPicker)}>
  <Plus className="h-4 w-4" />
</Button>

// Above the input area, show pinned context:
{pinnedContext.dateRange && (
  <div className="flex items-center gap-1 mb-1">
    <span className="text-xs bg-muted rounded px-2 py-0.5">
      {pinnedContext.dateRange.from} → {pinnedContext.dateRange.to}
      <button onClick={() => setPinnedContext({...pinnedContext, dateRange: undefined})} className="ml-1">×</button>
    </span>
  </div>
)}

// Render picker:
{showPicker && <ChatContextPicker onClose={() => setShowPicker(false)} />}
```

- [ ] **Step 3: Commit**

```bash
git add frontend/src/components/chat/chat-context-picker.tsx frontend/src/components/chat/chat-input.tsx
git commit -m "feat(chat): add context picker with date range selection"
```

---

## Task 15: Integration Test — End to End

**Files:** No new files — manual verification

- [ ] **Step 1: Start backend**

Run: `cd /Users/user/Tradstry && ./start.sh`
Expected: Backend starts, schema migrates to new version, no panics.

- [ ] **Step 2: Start frontend**

Run: `cd /Users/user/Tradstry/frontend && bun dev`
Expected: Next.js starts without build errors.

- [ ] **Step 3: Test chat flow in browser**

1. Click "Chat AI" → panel opens on the right, main content shrinks
2. Click "+" to create a new session
3. Type "What's my win rate?" and send
4. Observe streaming: tool_start indicator → analytics_calc runs → response streams in
5. Session title auto-generates
6. Close panel → click Chat AI → panel reopens with previous session

- [ ] **Step 4: Test context picker**

1. Click "+" button in chat input
2. Select a date range
3. Send a message → verify the date range is passed to the agent (check backend logs)

- [ ] **Step 5: Fix any issues found during testing**

Address compilation errors, runtime errors, or UI issues.

- [ ] **Step 6: Final commit**

```bash
git add -A
git commit -m "feat(chat): integration fixes from end-to-end testing"
```
