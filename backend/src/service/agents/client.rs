#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use rig::{client::CompletionClient, completion::Prompt, providers::openrouter};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::service::chat::types::{
    ChatEventBus, ChatStreamEnvelope, ChatStreamKind, LlmChatResponse, LlmMessage, LlmToolDef,
};

const DEFAULT_TEMPERATURE: f64 = 0.2;
const DEFAULT_MAX_TOKENS: u64 = 300_000;
const COOLDOWN: Duration = Duration::from_secs(60);
const OPENROUTER_CHAT_URL: &str = "https://openrouter.ai/api/v1/chat/completions";

/// Hardcoded model strategy in priority order.
/// nvidia first (both nemotron variants), then baidu, then poolside.
/// First model is tried first; on rate-limit or 5xx the client falls through
/// to the next model and cools the failed model down for 60s.
const HARDCODED_MODELS: &[&str] = &[
    "nvidia/nemotron-3-super-120b-a12b:free",
    "nvidia/nemotron-3-nano-omni-30b-a3b-reasoning:free",
    "baidu/cobuddy:free",
    "poolside/laguna-m.1:free",
    "poolside/laguna-xs.2:free",
];

#[derive(Clone, Debug)]
pub struct AgentsConfig {
    pub api_key: String,
    /// Ordered priority list of OpenRouter model slugs. The client tries them
    /// front-to-back, skipping any model currently cooled down.
    pub models: Vec<String>,
    pub preamble: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
    /// Optional reasoning effort: "low", "medium", or "high". When set, sent
    /// to OpenRouter as `reasoning: { effort }`. Reasoning text is broadcast
    /// to clients as a separate `Reasoning` stream kind.
    pub reasoning_effort: Option<String>,
}

impl AgentsConfig {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .context("OPENROUTER_API_KEY environment variable not set")?;

        let models: Vec<String> = HARDCODED_MODELS.iter().map(|s| (*s).to_owned()).collect();

        let preamble = std::env::var("OPENROUTER_PREAMBLE")
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());

        let reasoning_effort = std::env::var("OPENROUTER_REASONING_EFFORT")
            .ok()
            .map(|v| v.trim().to_lowercase())
            .filter(|v| matches!(v.as_str(), "low" | "medium" | "high"));

        Ok(Self {
            api_key,
            models,
            preamble,
            temperature: Some(DEFAULT_TEMPERATURE),
            max_tokens: Some(DEFAULT_MAX_TOKENS),
            reasoning_effort,
        })
    }
}

/// Tracks per-model rate-limit cooldowns so a flaky model is skipped for a
/// while instead of hammering it on every request.
#[derive(Default)]
struct CooldownMap {
    inner: Mutex<HashMap<String, Instant>>,
}

impl CooldownMap {
    fn is_cooled(&self, model: &str) -> bool {
        let guard = self.inner.lock().expect("cooldown map poisoned");
        guard
            .get(model)
            .map(|until| Instant::now() < *until)
            .unwrap_or(false)
    }

    fn cool(&self, model: &str) {
        let mut guard = self.inner.lock().expect("cooldown map poisoned");
        guard.insert(model.to_owned(), Instant::now() + COOLDOWN);
    }
}

#[derive(Clone)]
pub struct AgentsClient {
    or_client: openrouter::Client,
    config: AgentsConfig,
    http_client: reqwest::Client,
    cooldowns: std::sync::Arc<CooldownMap>,
}

impl AgentsClient {
    pub fn new(config: AgentsConfig) -> Result<Self> {
        let or_client = openrouter::Client::new(&config.api_key)
            .context("Failed to create OpenRouter client")?;

        Ok(Self {
            or_client,
            config,
            http_client: reqwest::Client::new(),
            cooldowns: std::sync::Arc::new(CooldownMap::default()),
        })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(AgentsConfig::from_env()?)
    }

    /// Primary (first-priority) model. Used as the recorded model name for
    /// artifacts/audits where a single value is needed.
    pub fn model(&self) -> &str {
        // models is non-empty by construction in AgentsConfig::from_env.
        self.config.models.first().map(String::as_str).unwrap_or("")
    }

    /// Display string for startup logging — the full configured strategy.
    pub fn models_display(&self) -> String {
        self.config.models.join(", ")
    }

    pub fn models(&self) -> &[String] {
        &self.config.models
    }

    pub fn preamble(&self) -> Option<&str> {
        self.config.preamble.as_deref()
    }

    /// Pick the first model in the strategy that isn't currently cooled down.
    /// Falls back to the first model if every model is cooled (better to try
    /// and fail than refuse outright).
    fn pick_models(&self) -> Vec<String> {
        let live: Vec<String> = self
            .config
            .models
            .iter()
            .filter(|m| !self.cooldowns.is_cooled(m))
            .cloned()
            .collect();
        if live.is_empty() {
            self.config.models.clone()
        } else {
            live
        }
    }

    /// One-shot prompt with multi-model failover. Used for background calls
    /// (title generation, memory extraction, retry helper).
    pub async fn prompt(&self, prompt: impl AsRef<str>) -> Result<String> {
        let prompt_text = prompt.as_ref();
        let mut last_err: Option<anyhow::Error> = None;

        for model in self.pick_models() {
            let mut agent_builder = self.or_client.agent(model.clone());

            if let Some(preamble) = self.config.preamble.as_deref() {
                agent_builder = agent_builder.preamble(preamble);
            }
            if let Some(temperature) = self.config.temperature {
                agent_builder = agent_builder.temperature(temperature);
            }
            if let Some(max_tokens) = self.config.max_tokens {
                agent_builder = agent_builder.max_tokens(max_tokens);
            }

            let agent = agent_builder.build();

            match agent.prompt(prompt_text).await {
                Ok(result) => return Ok(result),
                Err(e) => {
                    let msg = format!("{e}");
                    let is_transient = msg.contains("429")
                        || msg.contains("rate")
                        || msg.contains("503")
                        || msg.contains("502")
                        || msg.contains("500")
                        || msg.contains("timeout");
                    log::warn!(
                        "OpenRouter prompt failed for model {} (transient={}): {}",
                        model,
                        is_transient,
                        msg
                    );
                    if is_transient {
                        self.cooldowns.cool(&model);
                    }
                    last_err = Some(anyhow!("{e}"));
                    // Try next model
                    continue;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("OpenRouter prompt: no models attempted")))
    }

    /// Streaming chat with tool calls. Hits OpenRouter's OpenAI-compatible
    /// endpoint directly via reqwest so we can stream tokens, tool-call
    /// argument deltas, and reasoning deltas straight onto the broadcast bus.
    /// Failover happens only on pre-stream errors (HTTP error before any
    /// chunk has been broadcast); mid-stream failures surface to the client
    /// rather than silently switching models.
    pub async fn stream_chat(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[LlmToolDef]>,
        tx: ChatEventBus,
        job_id: &str,
        session_id: &str,
    ) -> Result<LlmChatResponse> {
        let mut last_err: Option<anyhow::Error> = None;

        for model in self.pick_models() {
            // Build the request body fresh per attempt (cheap; messages are borrowed).
            let mut body = serde_json::json!({
                "model": model,
                "messages": messages,
                "stream": true,
                "temperature": self.config.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            });

            if let Some(tools) = tools {
                body["tools"] = serde_json::to_value(tools)?;
            }

            if let Some(effort) = self.config.reasoning_effort.as_deref() {
                body["reasoning"] = serde_json::json!({ "effort": effort });
            }

            let response = self
                .http_client
                .post(OPENROUTER_CHAT_URL)
                .header("Authorization", format!("Bearer {}", self.config.api_key))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("OpenRouter network error on model {}: {}", model, e);
                    self.cooldowns.cool(&model);
                    last_err = Some(anyhow!("Failed to send request to OpenRouter: {e}"));
                    continue;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                let transient = status.as_u16() == 429 || status.is_server_error();
                log::warn!(
                    "OpenRouter HTTP {} on model {} (transient={}): {}",
                    status,
                    model,
                    transient,
                    text
                );
                if transient {
                    self.cooldowns.cool(&model);
                    last_err = Some(anyhow!("OpenRouter API error {}: {}", status, text));
                    continue;
                }
                return Err(anyhow!("OpenRouter API error {}: {}", status, text));
            }

            // We have a 2xx — commit to this model and consume the stream.
            return consume_stream(response, tx, job_id, session_id).await;
        }

        Err(last_err.unwrap_or_else(|| anyhow!("OpenRouter stream_chat: no models attempted")))
    }
}

/// Read SSE chunks from an OpenRouter streaming response, broadcasting Token,
/// Reasoning, and ToolStart envelopes as they arrive, and accumulating tool
/// call id/name/arguments. Returns either a ToolCall or TextComplete response.
async fn consume_stream(
    response: reqwest::Response,
    tx: ChatEventBus,
    job_id: &str,
    session_id: &str,
) -> Result<LlmChatResponse> {
    let mut full_text = String::new();
    let mut tool_call_id = String::new();
    let mut tool_call_name = String::new();
    let mut tool_call_arguments = String::new();
    let mut tool_name_sent = false;
    let mut is_tool_call = false;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Error reading stream chunk")?;
        let text = String::from_utf8_lossy(&chunk);
        buffer.push_str(&text);

        while let Some(newline_pos) = buffer.find('\n') {
            let line = buffer[..newline_pos].trim_end_matches('\r').to_owned();
            buffer = buffer[newline_pos + 1..].to_owned();

            if line.is_empty() {
                continue;
            }
            // OpenRouter emits ": OPENROUTER PROCESSING" comment heartbeats
            // on slow upstream models. Skip any SSE comment line.
            if line.starts_with(':') {
                continue;
            }

            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };

            if data == "[DONE]" {
                break;
            }

            let chunk_val: Value = match serde_json::from_str(data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let delta = &chunk_val["choices"][0]["delta"];
            if delta.is_null() {
                continue;
            }

            // Visible content tokens
            if let Some(content) = delta["content"].as_str()
                && !content.is_empty()
            {
                full_text.push_str(content);
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.to_owned(),
                    session_id: session_id.to_owned(),
                    kind: ChatStreamKind::Token,
                    content: Some(content.to_owned()),
                    tool_name: None,
                    message_id: None,
                });
            }

            // Reasoning tokens — surfaced as a separate kind so the UI can
            // render them in a "Thinking" disclosure rather than the body.
            if let Some(reasoning) = delta["reasoning"].as_str()
                && !reasoning.is_empty()
            {
                let _ = tx.send(ChatStreamEnvelope {
                    job_id: job_id.to_owned(),
                    session_id: session_id.to_owned(),
                    kind: ChatStreamKind::Reasoning,
                    content: Some(reasoning.to_owned()),
                    tool_name: None,
                    message_id: None,
                });
            }

            // Tool call deltas
            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                is_tool_call = true;
                for tc in tool_calls {
                    if let Some(id) = tc["id"].as_str()
                        && !id.is_empty()
                    {
                        tool_call_id = id.to_owned();
                    }

                    let func = &tc["function"];

                    if let Some(name) = func["name"].as_str()
                        && !name.is_empty()
                        && tool_call_name.is_empty()
                    {
                        tool_call_name = name.to_owned();

                        if !tool_name_sent {
                            tool_name_sent = true;
                            let _ = tx.send(ChatStreamEnvelope {
                                job_id: job_id.to_owned(),
                                session_id: session_id.to_owned(),
                                kind: ChatStreamKind::ToolStart,
                                content: None,
                                tool_name: Some(tool_call_name.clone()),
                                message_id: None,
                            });
                        }
                    }

                    if let Some(args) = func["arguments"].as_str() {
                        tool_call_arguments.push_str(args);
                    }
                }
            }
        }
    }

    if is_tool_call {
        Ok(LlmChatResponse::ToolCall {
            id: tool_call_id,
            name: tool_call_name,
            arguments: tool_call_arguments,
        })
    } else {
        Ok(LlmChatResponse::TextComplete {
            full_text: full_text.trim().to_owned(),
        })
    }
}

/// Translate our OpenAI-shaped messages/tools into a native Gemini
/// generateContent request body. Pure — unit tested.
fn build_gemini_request(
    messages: &[LlmMessage],
    tools: Option<&[LlmToolDef]>,
    preamble: Option<&str>,
    temperature: f64,
    max_tokens: u64,
) -> Value {
    let mut system_texts: Vec<String> = Vec::new();
    if let Some(p) = preamble
        && !p.is_empty()
    {
        system_texts.push(p.to_owned());
    }

    let mut contents: Vec<Value> = Vec::new();
    for m in messages {
        match m.role.as_str() {
            "system" => {
                if let Some(c) = &m.content
                    && !c.is_empty()
                {
                    system_texts.push(c.clone());
                }
            }
            "tool" => {
                let name = m.name.clone().unwrap_or_default();
                let raw = m.content.as_deref().unwrap_or("");
                // Gemini's functionResponse.response must be an object.
                let parsed = serde_json::from_str::<Value>(raw).unwrap_or(Value::Null);
                let response = if parsed.is_object() {
                    parsed
                } else {
                    json!({ "result": raw })
                };
                contents.push(json!({
                    "role": "user",
                    "parts": [{ "functionResponse": { "name": name, "response": response } }]
                }));
            }
            "assistant" => {
                let mut parts: Vec<Value> = Vec::new();
                if let Some(c) = &m.content
                    && !c.is_empty()
                {
                    parts.push(json!({ "text": c }));
                }
                if let Some(tcs) = &m.tool_calls {
                    for tc in tcs {
                        let args: Value = serde_json::from_str(&tc.function.arguments)
                            .unwrap_or_else(|_| json!({}));
                        parts.push(json!({
                            "functionCall": { "name": tc.function.name, "args": args }
                        }));
                    }
                }
                if !parts.is_empty() {
                    contents.push(json!({ "role": "model", "parts": parts }));
                }
            }
            _ => {
                // "user" and any unknown role → a user text turn.
                let text = m.content.clone().unwrap_or_default();
                contents.push(json!({ "role": "user", "parts": [{ "text": text }] }));
            }
        }
    }

    let mut body = json!({
        "contents": contents,
        "generationConfig": {
            "temperature": temperature,
            "maxOutputTokens": max_tokens,
            "thinkingConfig": { "includeThoughts": true }
        }
    });

    if !system_texts.is_empty() {
        body["systemInstruction"] = json!({ "parts": [{ "text": system_texts.join("\n\n") }] });
    }

    if let Some(tools) = tools {
        let decls: Vec<Value> = tools
            .iter()
            .map(|t| {
                json!({
                    "name": t.function.name,
                    "description": t.function.description,
                    "parameters": t.function.parameters,
                })
            })
            .collect();
        body["tools"] = json!([{ "functionDeclarations": decls }]);
    }

    body
}

#[cfg(test)]
mod tests {
    use super::build_gemini_request;
    use crate::service::chat::types::{
        LlmFunctionCall, LlmFunctionDef, LlmMessage, LlmToolCall, LlmToolDef,
    };
    use serde_json::json;

    fn msg(role: &str, content: Option<&str>) -> LlmMessage {
        LlmMessage {
            role: role.to_owned(),
            content: content.map(|c| c.to_owned()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    #[test]
    fn builds_user_and_assistant_contents_with_correct_roles() {
        let messages = vec![msg("user", Some("hello")), msg("assistant", Some("hi there"))];
        let body = build_gemini_request(&messages, None, None, 0.2, 100);
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "hello");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[1]["parts"][0]["text"], "hi there");
        assert_eq!(body["generationConfig"]["thinkingConfig"]["includeThoughts"], true);
    }

    #[test]
    fn merges_system_message_and_preamble_into_system_instruction() {
        let messages = vec![msg("system", Some("be terse")), msg("user", Some("hi"))];
        let body = build_gemini_request(&messages, None, Some("you are a bot"), 0.2, 100);
        let sys = body["systemInstruction"]["parts"][0]["text"].as_str().unwrap();
        assert!(sys.contains("you are a bot"));
        assert!(sys.contains("be terse"));
        assert_eq!(body["contents"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn maps_assistant_tool_calls_to_function_call_parts() {
        let messages = vec![LlmMessage {
            role: "assistant".to_owned(),
            content: None,
            tool_calls: Some(vec![LlmToolCall {
                id: "x".to_owned(),
                call_type: "function".to_owned(),
                function: LlmFunctionCall {
                    name: "get_trades".to_owned(),
                    arguments: "{\"symbol\":\"AAPL\"}".to_owned(),
                },
            }]),
            tool_call_id: None,
            name: None,
        }];
        let body = build_gemini_request(&messages, None, None, 0.2, 100);
        let part = &body["contents"][0]["parts"][0]["functionCall"];
        assert_eq!(part["name"], "get_trades");
        assert_eq!(part["args"]["symbol"], "AAPL");
        assert_eq!(body["contents"][0]["role"], "model");
    }

    #[test]
    fn maps_tool_result_to_function_response() {
        let messages = vec![LlmMessage {
            role: "tool".to_owned(),
            content: Some("{\"count\":3}".to_owned()),
            tool_calls: None,
            tool_call_id: Some("call_1".to_owned()),
            name: Some("get_trades".to_owned()),
        }];
        let body = build_gemini_request(&messages, None, None, 0.2, 100);
        let fr = &body["contents"][0]["parts"][0]["functionResponse"];
        assert_eq!(fr["name"], "get_trades");
        assert_eq!(fr["response"]["count"], 3);
        assert_eq!(body["contents"][0]["role"], "user");
    }

    #[test]
    fn maps_tools_to_function_declarations() {
        let tools = vec![LlmToolDef {
            tool_type: "function".to_owned(),
            function: LlmFunctionDef {
                name: "get_trades".to_owned(),
                description: "fetch trades".to_owned(),
                parameters: json!({ "type": "object" }),
            },
        }];
        let messages: Vec<LlmMessage> = vec![];
        let body = build_gemini_request(&messages, Some(&tools), None, 0.2, 100);
        let decl = &body["tools"][0]["functionDeclarations"][0];
        assert_eq!(decl["name"], "get_trades");
        assert_eq!(decl["description"], "fetch trades");
    }
}
