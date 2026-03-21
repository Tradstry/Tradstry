# Chat Agent System Design

## Overview

A persistent chat system where users ask natural language questions about their trading data. An async Rust ReAct loop orchestrates tool calls — DB queries, hybrid semantic search, and analytics calculations — then streams responses via WebSocket.

## Requirements

- Persistent side panel (not overlay) — user can chat while using the rest of the app
- Multiple named conversations with auto-generated titles
- Streaming responses via WebSocket (existing subscription infrastructure)
- Context picker: users can pin specific trades, date ranges, and playbooks
- Three agent tools: `db_query`, `semantic_search`, `analytics_calc`
- Hybrid search: dense vectors (Jina) + sparse vectors (BM25) → RRF fusion → Jina reranker
- LLM provider: Groq (existing `AgentsClient`)
- Max 5 tool-call iterations per message (prevents runaway ReAct loops)

---

## 1. Database Schema

### `chat_sessions`

| Column     | Type         | Notes                    |
|------------|--------------|--------------------------|
| id         | TEXT PK      | UUID                     |
| user_id    | TEXT NOT NULL | FK to users              |
| account_id | TEXT NOT NULL | FK to accounts           |
| title      | TEXT         | Auto-generated from first message |
| created_at | TEXT NOT NULL | ISO timestamp            |
| updated_at | TEXT NOT NULL | ISO timestamp            |

### `chat_messages`

| Column       | Type         | Notes                                          |
|--------------|--------------|-------------------------------------------------|
| id           | TEXT PK      | UUID                                            |
| session_id   | TEXT NOT NULL | FK to chat_sessions                             |
| role         | TEXT NOT NULL | "user" \| "assistant" \| "tool"                 |
| content      | TEXT NOT NULL | Message text or tool result JSON                |
| context_json | TEXT         | Pinned trades/date ranges (user messages only)  |
| tool_name    | TEXT         | Which tool was called (tool messages only)       |
| created_at   | TEXT NOT NULL | ISO timestamp                                   |

---

## 2. Backend Agent Graph (LangGraph ReAct Loop)

### Extending AgentsClient for Streaming + Tool Calling

The current `AgentsClient` only exposes `prompt() -> String` via `rig-core`. For the chat agent we need:

1. **Streaming:** Add a `stream_chat()` method that makes raw HTTP requests to the Groq chat completions endpoint (`POST /openai/v1/chat/completions` with `stream: true`). Groq's API is OpenAI-compatible, so we use `reqwest` with SSE parsing (read `data:` lines, deserialize `ChatCompletionChunk`). This bypasses `rig`'s non-streaming `Prompt` trait.

2. **Tool/Function Calling:** Groq supports OpenAI-compatible function calling. The `stream_chat()` method accepts a `tools` parameter (JSON array of tool schemas). The LLM response includes `tool_calls` in the delta when it wants to invoke a tool, or `content` tokens for the final answer. We parse both from the SSE stream.

3. **Message History:** The method accepts `Vec<ChatMessage>` (system/user/assistant/tool roles) so the ReAct loop can pass the full conversation state.

```rust
impl AgentsClient {
    /// Existing method — unchanged
    pub async fn prompt(&self, prompt: impl AsRef<str>) -> Result<String>;

    /// New: streaming chat with tool support
    pub async fn stream_chat(
        &self,
        messages: Vec<GroqMessage>,
        tools: Option<Vec<GroqToolDef>>,
        tx: broadcast::Sender<ChatStreamEnvelope>,
    ) -> Result<GroqChatResponse>;
    // GroqChatResponse = either ToolCall { name, arguments } or TextComplete { full_text }
}
```

### ReAct Loop Implementation (Async Rust Loop)

The LangGraph crate's `StateNodeAction` is synchronous, which is incompatible with the async I/O required by every node (Groq HTTP, Qdrant, Turso). Rather than fight this, **the ReAct loop is implemented as a plain async Rust function** — no `StateGraph` runner. We still use LangGraph's state primitives (`Topic`, `LastValue`) for state management if needed, but the control flow is a simple async loop.

```rust
async fn run_chat_agent(
    state: ChatState,
    agents: AgentsClient,
    turso: TursoClient,
    qdrant: VectorDatabaseClient,
    tx: broadcast::Sender<ChatStreamEnvelope>,
) -> Result<ChatAgentResult> {
    let mut state = state;
    loop {
        // 1. Call LLM (streaming)
        let response = agents.stream_chat(&state.messages, &TOOL_SCHEMAS, tx.clone()).await?;
        match response {
            GroqChatResponse::ToolCall { name, arguments } => {
                if state.iteration_count >= 5 {
                    // Force final answer — re-call LLM without tools
                    let final_resp = agents.stream_chat(&state.messages, None, tx.clone()).await?;
                    break Ok(ChatAgentResult::from(final_resp));
                }
                // 2. Execute tool
                let result = execute_tool(&name, &arguments, &state, &turso, &qdrant).await?;
                state.messages.push(tool_message(&name, &result));
                state.iteration_count += 1;
                tx.send(ChatStreamEnvelope::tool_result(&name, &result))?;
                // 3. Loop back to LLM
            }
            GroqChatResponse::TextComplete { full_text, message_id } => {
                break Ok(ChatAgentResult { text: full_text, message_id });
            }
        }
    }
}
```

This is simpler, fully async, and achieves the same ReAct pattern. The LangGraph crate can be used for more complex multi-agent workflows in the future once async node support is added.

### State

```rust
struct ChatState {
    messages: Vec<ChatMessage>,       // conversation history (user/assistant/tool)
    user_context: Option<UserContext>, // pinned trades, date range, playbook IDs
    user_id: String,
    account_id: String,
    iteration_count: u32,             // tracks tool loops, max 5
}
```

### Title Generation

Runs as a separate `tokio::spawn` task in parallel with the ReAct loop on the first message of a new session. Makes a non-streaming `prompt()` call: "Generate a short title (≤10 words) for this conversation: {user_message}". Updates the session title via DB write. If it fails, falls back to the first 50 chars of the user's message.

### Conversation History Truncation

When building the messages array for the LLM, truncate by **user turns** (not raw message count): keep the last 5 user messages plus their full tool-call chains (assistant tool_call → tool result → ... → assistant final). This preserves the context of each turn's reasoning. The system prompt and pinned context are always included.

### Tool Definitions

The LLM receives these as OpenAI-compatible function schemas in the `tools` parameter:

| Tool              | Structured Input (JSON from LLM)                          | What it does                                                                                                              |
|-------------------|------------------------------------------------------------|---------------------------------------------------------------------------------------------------------------------------|
| `db_query`        | `{ "entity": "trades"\|"journal"\|"playbook", "filters": { "symbol?": str, "date_from?": str, "date_to?": str, "direction?": str }, "limit?": int }` | Maps to a parameterized SQL query against Turso. Fixed query templates per entity type. No raw SQL from the LLM — the `entity` + `filters` object maps to a predefined query builder with whitelisted columns and operators. Scoping: `trades` and `journal` filter by `user_id + account_id`; `playbook` filters by `user_id` only (playbooks table has no `account_id` column). |
| `semantic_search` | `{ "query": str, "date_from?": str, "date_to?": str }`    | Hybrid search on Qdrant: dense + BM25 sparse, fused with RRF, reranked with Jina. `user_id` and `account_id` injected from state (not from LLM). Returns top-K relevant chunks. |
| `analytics_calc`  | `{ "metrics": ["win_rate"\|"total_pnl"\|"avg_r"\|"profit_factor"\|"streak"\|"per_symbol"], "filters": { "symbol?": str, "date_from?": str, "date_to?": str } }` | Computes stats in Rust. Pure computation, no LLM needed. Always scoped by `user_id + account_id` from state (not from LLM input). |

### Streaming

The `llm_node` calls `stream_chat()` which:
1. Opens an SSE connection to Groq
2. For each `data:` chunk, parses the delta
3. If `delta.content` is present → sends `ChatStreamEnvelope { kind: "token", content }` to a **dedicated** `chat_events_tx: broadcast::Sender<ChatStreamEnvelope>` channel (separate from the existing `ai_events_tx` which carries `AiEventEnvelope`)
4. If `delta.tool_calls` is present → sends `ChatStreamEnvelope { kind: "tool_start", tool_name }` and accumulates the tool call arguments
5. On stream end → sends `ChatStreamEnvelope { kind: "done" }`

The new `chat_events_tx` channel is created in `main.rs` alongside the existing `ai_events_tx`, and passed to the GraphQL context as `ChatEventBus`.

---

## 3. GraphQL API Layer

### Queries

```graphql
chatSessions(accountId: String!, limit: Int): [ChatSession!]!
chatMessages(sessionId: String!, limit: Int, before: String): [ChatMessage!]!
# `before` is a cursor (message ID). Implementation: look up cursor's created_at,
# then WHERE created_at < cursor_created_at ORDER BY created_at DESC LIMIT n.
# UUIDs are not ordered, so cursor resolution requires the timestamp lookup.
# `limit` defaults to 50.
```

### Mutations

```graphql
createChatSession(accountId: String!): ChatSession!
# user_id extracted from Clerk JWT; account_id passed explicitly
# (matches existing pattern: refresh_ai_insights, journal mutations all take account_id)

updateChatSession(sessionId: String!, title: String!): ChatSession!

deleteChatSession(sessionId: String!): Boolean!
# Validates session ownership: session.user_id must match JWT user_id

sendChatMessage(
  sessionId: String!
  content: String!
  context: ChatContextInput
): String!  # returns a job_id for the subscription
# user_id from JWT, account_id from the session's stored account_id
```

### Subscriptions

```graphql
chatStream(jobId: String!): ChatStreamEvent!
```

### Types

```graphql
type ChatSession {
  id: String!
  title: String
  createdAt: String!
  updatedAt: String!
}

type ChatMessage {
  id: String!
  sessionId: String!
  role: String!
  content: String!
  contextJson: String
  toolName: String
  createdAt: String!
}

type ChatStreamEvent {
  jobId: String!
  sessionId: String!
  kind: String!        # "token" | "tool_start" | "tool_result" | "done" | "error"
  content: String
  toolName: String
  messageId: String    # included in "done" event — the persisted assistant message ID
                       # so frontend can add it to React Query cache without refetching
}

input ChatContextInput {
  tradeIds: [String!]
  dateRange: DateRangeInput
  playbookIds: [String!]
}

input DateRangeInput {
  from: String!
  to: String!
}
```

### Flow

1. `sendChatMessage` persists the user message, spawns the LangGraph agent as a background task, returns a `job_id`
2. Frontend subscribes to `chatStream(jobId)` to receive streaming tokens and tool events
3. When the agent finishes, the full assistant message is persisted to `chat_messages`

---

## 4. Frontend Architecture

### Layout

```
┌──────────┬────────────┐
│ Sidebar  │  Main      │   (chat closed)
│          │  Content   │
└──────────┴────────────┘

┌──────────┬────────┬───────┐
│ Sidebar  │ Main   │ Chat  │   (chat open)
│          │Content │ Panel │
└──────────┴────────┴───────┘
```

Chat panel width: ~380px fixed, with a collapse button. "Chat AI" button in `site-header.tsx` toggles the panel.

### Components

| Component           | Purpose                                                                                              |
|---------------------|------------------------------------------------------------------------------------------------------|
| `ChatPanel`         | Side panel container. Holds session list header + active conversation.                               |
| `ChatSessionList`   | Dropdown/popover to switch between conversations or create new one.                                  |
| `ChatMessageList`   | Scrollable message area. Renders user/assistant/tool messages differently.                           |
| `ChatInput`         | Text input + "+" context picker button + send button.                                                |
| `ChatContextPicker` | Popover from "+" button. Tabs: Trades (searchable), Date Range (date picker), Playbooks (list). Pinned items shown as badges above input. |
| `ChatStreamMessage` | Renders in-progress assistant message with streaming tokens + tool call indicators.                  |

### State Management

- `useChatSessions()` — React Query hook, fetches session list
- `useChatMessages(sessionId)` — React Query hook, fetches message history
- `useSendMessage()` — mutation hook, sends message + subscribes to stream
- Zustand store for chat UI state: `isOpen`, `activeSessionId`, `pinnedContext`, `streamingMessage`

---

## 5. Hybrid Search Pipeline

### Qdrant Collection Setup

The existing collection uses a single `"dense"` named vector. For hybrid search, we create a **new collection** `tradstry_hybrid` with both dense and sparse vectors:

```
Collection: tradstry_hybrid
├── Named vector "dense": Jina embedding (dim from model, cosine distance)
└── Named sparse vector "sparse": BM25 weights
```

The existing collection remains untouched for backward compatibility with current AI features.

### BM25 Sparse Vector Generation

Qdrant stores and searches sparse vectors but does not generate them. We tokenize and compute BM25 weights **in Rust** using a lightweight approach:
- Tokenize text (lowercase, split on whitespace/punctuation, remove stopwords)
- Compute term frequencies per document
- Convert to sparse vector format: `{ indices: [token_hash_1, token_hash_2, ...], values: [tf_weight_1, tf_weight_2, ...] }`
- Use FNV hashing to map tokens to integer indices (Qdrant sparse vectors use integer indices)

For IDF (inverse document frequency): start with TF-only for simplicity. BM25 IDF requires corpus-level term frequency stats that would need periodic recomputation. TF-only sparse vectors still provide exact keyword matching — the dense vectors handle semantic relevance. If retrieval quality is insufficient, add IDF as a follow-up by maintaining a term frequency table updated on each upsert.

This runs in the backend's indexing pipeline. No external service needed.

### Indexing (write path)

When trades/journal entries are created or updated:

1. Build text chunk from trade data (symbol, direction, P&L, notes, tags, journal text)
2. Generate dense vector via Jina embeddings (existing `VectorDatabaseClient`)
3. Generate sparse vector via BM25 tokenizer (new Rust module in `vector_database/`)
4. Upsert to Qdrant `tradstry_hybrid` collection with both vectors + payload: `user_id`, `account_id`, `source_type`, `source_id`, `created_at`

### Querying (read path — inside `semantic_search` tool)

```
1. User query arrives with filters (user_id, optional date range)
       ↓
2. Embed the query via Jina → dense vector
   Tokenize the query for BM25 → sparse vector
       ↓
3. Qdrant Query API with prefetch:
   ├── prefetch[0]: dense vector search (query embedding), limit 20
   └── prefetch[1]: sparse vector search (query sparse vector), limit 20
       ↓
4. Fusion via RRF (built into Qdrant's query API)
       ↓
5. Top 10 results returned
       ↓
6. Jina reranker scores the 10 results against the original query text
       ↓
7. Return top 5 reranked results to the LLM
```

### Required Payload Indexes

- `user_id` — keyword index
- `account_id` — keyword index
- `source_type` — keyword index
- `created_at` — integer/datetime index

---

## 6. Error Handling & Edge Cases

| Scenario                                | Handling                                                                                                 |
|-----------------------------------------|----------------------------------------------------------------------------------------------------------|
| Groq API timeout/error                  | Retry once with exponential backoff (`tokio::time::sleep` in the async loop). If still fails, stream error event. |
| Tool execution fails                    | Tool returns error as observation. LLM sees it and can retry with different params or inform the user.   |
| Empty search results                    | LLM sees "no results found" and responds naturally.                                                      |
| User sends message while streaming      | Queue it. Frontend disables send button while streaming is active.                                       |
| Chat session deleted while streaming    | Broadcast channel drops receiver. Backend task stops gracefully.                                         |
| Context picker — 50+ trades pinned      | Cap at 20 trade IDs. Suggest date range filter instead. Pinned data is summarized for LLM context.      |
| Very long conversation history          | Truncate to last 5 user turns (~20 raw messages including tool chains) sent to LLM. Full history stays in DB for scrolling. |
| Title generation fails                  | Fallback to first 50 chars of user's first message.                                                      |
| Runaway ReAct loop                      | Hard cap at 5 tool-call iterations per message. At limit, LLM must synthesize with what it has.          |
