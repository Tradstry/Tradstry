#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use rig::{client::CompletionClient, completion::Prompt, providers::gemini};
use serde_json::{Value, json};
use std::time::Duration;
use uuid::Uuid;

use crate::service::ai::chat::types::{
    ChatEventBus, ChatStreamEnvelope, ChatStreamKind, LlmChatResponse, LlmMessage, LlmToolDef,
};

const DEFAULT_TEMPERATURE: f64 = 0.2;
const DEFAULT_MAX_TOKENS: u64 = 300_000;
const MODEL: &str = "gemini-3.5-flash";
const MAX_RETRIES: u32 = 3;
/// Hard ceiling on a single one-shot prompt call so a stalled upstream connection
/// can never hang a request indefinitely (the streaming path bounds itself).
const PROMPT_TIMEOUT_SECS: u64 = 90;
const GEMINI_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta/models";
/// Resumable-upload entrypoint for the Gemini Files API.
const GEMINI_FILES_UPLOAD_URL: &str =
    "https://generativelanguage.googleapis.com/upload/v1beta/files";
/// Base for polling a file's status; the returned `file.name` (e.g. `files/abc`)
/// is appended directly.
const GEMINI_FILES_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
/// Cap on how long we wait for an uploaded video to finish processing.
const FILE_ACTIVE_TIMEOUT_SECS: u64 = 60;
/// How often we re-check the file's processing state.
const FILE_POLL_INTERVAL_SECS: u64 = 2;

#[derive(Clone, Debug)]
pub struct AgentsConfig {
    pub api_key: String,
    pub preamble: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
}

impl AgentsConfig {
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("GEMINI_API_KEY")
            .context("GEMINI_API_KEY environment variable not set")?;

        let preamble = std::env::var("GEMINI_PREAMBLE")
            .ok()
            .map(|v| v.trim().to_owned())
            .filter(|v| !v.is_empty());

        Ok(Self {
            api_key,
            preamble,
            temperature: Some(DEFAULT_TEMPERATURE),
            max_tokens: Some(DEFAULT_MAX_TOKENS),
        })
    }
}

/// True if an error message looks transient (rate limit / 5xx / network).
fn is_transient(msg: &str) -> bool {
    msg.contains("429")
        || msg.contains("rate")
        || msg.contains("503")
        || msg.contains("502")
        || msg.contains("500")
        || msg.contains("timeout")
}

#[derive(Clone)]
pub struct AgentsClient {
    gemini_client: gemini::Client,
    config: AgentsConfig,
    http_client: reqwest::Client,
}

impl AgentsClient {
    pub fn new(config: AgentsConfig) -> Result<Self> {
        let gemini_client =
            gemini::Client::new(&config.api_key).context("Failed to create Gemini client")?;

        Ok(Self {
            gemini_client,
            config,
            http_client: reqwest::Client::new(),
        })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(AgentsConfig::from_env()?)
    }

    /// The model name, for artifacts/audits and startup logging.
    pub fn model(&self) -> &str {
        MODEL
    }

    /// Display string for startup logging.
    pub fn models_display(&self) -> String {
        MODEL.to_owned()
    }

    pub fn preamble(&self) -> Option<&str> {
        self.config.preamble.as_deref()
    }

    /// One-shot prompt via the rig Gemini agent, with simple transient retry.
    /// Used for background calls (title generation, memory extraction, insights).
    pub async fn prompt(&self, prompt: impl AsRef<str>) -> Result<String> {
        let prompt_text = prompt.as_ref();
        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 0..MAX_RETRIES {
            let mut agent_builder = self.gemini_client.agent(MODEL);
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

            let timed = tokio::time::timeout(
                Duration::from_secs(PROMPT_TIMEOUT_SECS),
                agent.prompt(prompt_text),
            )
            .await;

            match timed {
                Ok(Ok(result)) => return Ok(result),
                Ok(Err(e)) => {
                    let msg = format!("{e}");
                    log::warn!("Gemini prompt failed (attempt {}): {}", attempt + 1, msg);
                    last_err = Some(anyhow!("{e}"));
                    if is_transient(&msg) && attempt + 1 < MAX_RETRIES {
                        tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    break;
                }
                Err(_elapsed) => {
                    log::warn!(
                        "Gemini prompt timed out after {PROMPT_TIMEOUT_SECS}s (attempt {})",
                        attempt + 1
                    );
                    last_err = Some(anyhow!(
                        "Gemini prompt timed out after {PROMPT_TIMEOUT_SECS}s"
                    ));
                    if attempt + 1 < MAX_RETRIES {
                        tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    break;
                }
            }
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Gemini prompt: no attempts made")))
    }

    /// Streaming chat with tool calls against Gemini's native streamGenerateContent
    /// (SSE). Retries only on pre-stream failures (network / HTTP error before any
    /// chunk); once a 2xx stream begins, errors surface to the client.
    pub async fn stream_chat(
        &self,
        messages: &[LlmMessage],
        tools: Option<&[LlmToolDef]>,
        tx: ChatEventBus,
        job_id: &str,
        session_id: &str,
    ) -> Result<LlmChatResponse> {
        let url = format!("{GEMINI_BASE_URL}/{MODEL}:streamGenerateContent?alt=sse");
        let body = build_gemini_request(
            messages,
            tools,
            self.config.preamble.as_deref(),
            self.config.temperature.unwrap_or(DEFAULT_TEMPERATURE),
            self.config.max_tokens.unwrap_or(DEFAULT_MAX_TOKENS),
        );

        let mut last_err: Option<anyhow::Error> = None;

        for attempt in 0..MAX_RETRIES {
            let response = self
                .http_client
                .post(&url)
                .header("x-goog-api-key", &self.config.api_key)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await;

            let response = match response {
                Ok(r) => r,
                Err(e) => {
                    log::warn!("Gemini network error (attempt {}): {}", attempt + 1, e);
                    last_err = Some(anyhow!("Failed to send request to Gemini: {e}"));
                    if attempt + 1 < MAX_RETRIES {
                        tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                        continue;
                    }
                    break;
                }
            };

            if !response.status().is_success() {
                let status = response.status();
                let text = response.text().await.unwrap_or_default();
                let transient = status.as_u16() == 429 || status.is_server_error();
                log::warn!("Gemini HTTP {} (attempt {}): {}", status, attempt + 1, text);
                last_err = Some(anyhow!("Gemini API error {}: {}", status, text));
                if transient && attempt + 1 < MAX_RETRIES {
                    tokio::time::sleep(Duration::from_millis(250 * (attempt as u64 + 1))).await;
                    continue;
                }
                return Err(anyhow!("Gemini API error {}: {}", status, text));
            }

            // 2xx — commit to this stream.
            return consume_stream(response, tx, job_id, session_id).await;
        }

        Err(last_err.unwrap_or_else(|| anyhow!("Gemini stream_chat: no attempts made")))
    }

    /// Upload bytes to the Gemini Files API and return the file `uri` (usable as a
    /// file_data part). Used for video. Waits until the file is ACTIVE.
    pub async fn upload_file(&self, bytes: Vec<u8>, mime_type: &str) -> anyhow::Result<String> {
        let api_key = &self.config.api_key;
        let len = bytes.len();

        // 1. Start a resumable upload session.
        let start_url = format!("{GEMINI_FILES_UPLOAD_URL}?key={api_key}");
        let start_resp = self
            .http_client
            .post(&start_url)
            .header("X-Goog-Upload-Protocol", "resumable")
            .header("X-Goog-Upload-Command", "start")
            .header("X-Goog-Upload-Header-Content-Length", len.to_string())
            .header("X-Goog-Upload-Header-Content-Type", mime_type)
            .header("Content-Type", "application/json")
            .json(&json!({ "file": { "display_name": "notebook-video" } }))
            .send()
            .await
            .context("Gemini Files API: failed to start resumable upload")?;

        if !start_resp.status().is_success() {
            let status = start_resp.status();
            let text = start_resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Gemini Files API start failed {}: {}",
                status,
                text
            ));
        }

        let upload_url = start_resp
            .headers()
            .get("X-Goog-Upload-URL")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned())
            .context("Gemini Files API: missing X-Goog-Upload-URL response header")?;

        // 2. Upload the bytes and finalize in a single request.
        let upload_resp = self
            .http_client
            .post(&upload_url)
            .header("X-Goog-Upload-Command", "upload, finalize")
            .header("X-Goog-Upload-Offset", "0")
            .header("Content-Length", len.to_string())
            .body(bytes)
            .send()
            .await
            .context("Gemini Files API: failed to upload/finalize bytes")?;

        if !upload_resp.status().is_success() {
            let status = upload_resp.status();
            let text = upload_resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Gemini Files API upload failed {}: {}",
                status,
                text
            ));
        }

        let upload_json: Value = upload_resp
            .json()
            .await
            .context("Gemini Files API: failed to parse upload response JSON")?;

        let file = &upload_json["file"];
        let file_name = file
            .get("name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .context("Gemini Files API: upload response missing file.name")?;
        let file_uri = file
            .get("uri")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned())
            .context("Gemini Files API: upload response missing file.uri")?;
        let mut state = file
            .get("state")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();

        if state == "ACTIVE" {
            return Ok(file_uri);
        }

        // 3. Poll until ACTIVE (videos process asynchronously).
        let get_url = format!("{GEMINI_FILES_BASE_URL}/{file_name}?key={api_key}");
        let deadline = tokio::time::Instant::now() + Duration::from_secs(FILE_ACTIVE_TIMEOUT_SECS);

        while state != "ACTIVE" {
            if state == "FAILED" {
                return Err(anyhow!(
                    "Gemini Files API: file {} processing FAILED",
                    file_name
                ));
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(anyhow!(
                    "Gemini Files API: file {} did not become ACTIVE within {}s (last state: {})",
                    file_name,
                    FILE_ACTIVE_TIMEOUT_SECS,
                    state
                ));
            }

            tokio::time::sleep(Duration::from_secs(FILE_POLL_INTERVAL_SECS)).await;

            let poll_resp = self
                .http_client
                .get(&get_url)
                .send()
                .await
                .context("Gemini Files API: failed to poll file status")?;

            if !poll_resp.status().is_success() {
                let status = poll_resp.status();
                let text = poll_resp.text().await.unwrap_or_default();
                return Err(anyhow!("Gemini Files API poll failed {}: {}", status, text));
            }

            let poll_json: Value = poll_resp
                .json()
                .await
                .context("Gemini Files API: failed to parse poll response JSON")?;

            // The GET <file.name> endpoint returns the file object directly.
            state = poll_json
                .get("state")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_owned();
        }

        Ok(file_uri)
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
                        let mut part = json!({
                            "functionCall": { "name": tc.function.name, "args": args }
                        });
                        // Echo back the thinking signature Gemini returned, or it
                        // rejects the replayed call with 400 INVALID_ARGUMENT.
                        if let Some(sig) = &tc.thought_signature {
                            part["thoughtSignature"] = json!(sig);
                        }
                        parts.push(part);
                    }
                }
                if !parts.is_empty() {
                    contents.push(json!({ "role": "model", "parts": parts }));
                }
            }
            _ => {
                // "user" and any unknown role → a user turn, possibly multimodal.
                let mut parts: Vec<Value> = Vec::new();
                if let Some(text) = &m.content
                    && !text.is_empty()
                {
                    parts.push(json!({ "text": text }));
                }
                if let Some(media) = &m.media {
                    for part in media {
                        if let Some(b64) = &part.data {
                            parts.push(json!({
                                "inline_data": { "mime_type": part.mime_type, "data": b64 }
                            }));
                        } else if let Some(uri) = &part.file_uri {
                            parts.push(json!({
                                "file_data": { "mime_type": part.mime_type, "file_uri": uri }
                            }));
                        }
                    }
                }
                if parts.is_empty() {
                    parts.push(json!({ "text": "" }));
                }
                contents.push(json!({ "role": "user", "parts": parts }));
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

/// A single classified Gemini response part. Pure — unit tested.
#[derive(Debug, PartialEq)]
enum GeminiPart {
    Text(String),
    Thought(String),
    FunctionCall {
        name: String,
        args: Value,
        thought_signature: Option<String>,
    },
    Other,
}

fn classify_gemini_part(part: &Value) -> GeminiPart {
    if let Some(fc) = part.get("functionCall") {
        let name = fc
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
        let args = fc.get("args").cloned().unwrap_or_else(|| json!({}));
        // Gemini attaches the thinking signature to the part holding the call.
        let thought_signature = part
            .get("thoughtSignature")
            .and_then(|v| v.as_str())
            .map(|s| s.to_owned());
        return GeminiPart::FunctionCall {
            name,
            args,
            thought_signature,
        };
    }
    if let Some(text) = part.get("text").and_then(|v| v.as_str()) {
        let is_thought = part
            .get("thought")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        return if is_thought {
            GeminiPart::Thought(text.to_owned())
        } else {
            GeminiPart::Text(text.to_owned())
        };
    }
    GeminiPart::Other
}

/// Read SSE chunks from Gemini's streamGenerateContent response, broadcasting
/// Token, Reasoning, and ToolStart envelopes as they arrive, and accumulating a
/// single function call. Returns either a ToolCall or TextComplete response.
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
    let mut tool_call_signature: Option<String> = None;
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

            if line.is_empty() || line.starts_with(':') {
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

            let Some(parts) = chunk_val["candidates"][0]["content"]["parts"].as_array() else {
                continue;
            };

            for part in parts {
                match classify_gemini_part(part) {
                    GeminiPart::Text(content) if !content.is_empty() => {
                        full_text.push_str(&content);
                        let _ = tx.send(ChatStreamEnvelope {
                            job_id: job_id.to_owned(),
                            session_id: session_id.to_owned(),
                            kind: ChatStreamKind::Token,
                            content: Some(content),
                            tool_name: None,
                            message_id: None,
                        });
                    }
                    GeminiPart::Thought(reasoning) if !reasoning.is_empty() => {
                        let _ = tx.send(ChatStreamEnvelope {
                            job_id: job_id.to_owned(),
                            session_id: session_id.to_owned(),
                            kind: ChatStreamKind::Reasoning,
                            content: Some(reasoning),
                            tool_name: None,
                            message_id: None,
                        });
                    }
                    GeminiPart::FunctionCall {
                        name,
                        args,
                        thought_signature,
                    } => {
                        is_tool_call = true;
                        if tool_call_name.is_empty() {
                            tool_call_name = name;
                            tool_call_id = format!("call_{}", Uuid::new_v4());
                            if thought_signature.is_some() {
                                tool_call_signature = thought_signature;
                            }
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
                        tool_call_arguments = args.to_string();
                    }
                    _ => {}
                }
            }
        }
    }

    if is_tool_call {
        Ok(LlmChatResponse::ToolCall {
            id: tool_call_id,
            name: tool_call_name,
            arguments: tool_call_arguments,
            thought_signature: tool_call_signature,
        })
    } else {
        Ok(LlmChatResponse::TextComplete {
            full_text: full_text.trim().to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{GeminiPart, build_gemini_request, classify_gemini_part};
    use crate::service::ai::chat::types::{
        LlmFunctionCall, LlmFunctionDef, LlmMediaPart, LlmMessage, LlmToolCall, LlmToolDef,
    };
    use serde_json::json;

    fn msg(role: &str, content: Option<&str>) -> LlmMessage {
        LlmMessage {
            role: role.to_owned(),
            content: content.map(|c| c.to_owned()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            media: None,
        }
    }

    #[test]
    fn builds_user_and_assistant_contents_with_correct_roles() {
        let messages = vec![
            msg("user", Some("hello")),
            msg("assistant", Some("hi there")),
        ];
        let body = build_gemini_request(&messages, None, None, 0.2, 100);
        let contents = body["contents"].as_array().unwrap();
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0]["role"], "user");
        assert_eq!(contents[0]["parts"][0]["text"], "hello");
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[1]["parts"][0]["text"], "hi there");
        assert_eq!(
            body["generationConfig"]["thinkingConfig"]["includeThoughts"],
            true
        );
    }

    #[test]
    fn merges_system_message_and_preamble_into_system_instruction() {
        let messages = vec![msg("system", Some("be terse")), msg("user", Some("hi"))];
        let body = build_gemini_request(&messages, None, Some("you are a bot"), 0.2, 100);
        let sys = body["systemInstruction"]["parts"][0]["text"]
            .as_str()
            .unwrap();
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
                thought_signature: None,
            }]),
            tool_call_id: None,
            name: None,
            media: None,
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
            media: None,
        }];
        let body = build_gemini_request(&messages, None, None, 0.2, 100);
        let fr = &body["contents"][0]["parts"][0]["functionResponse"];
        assert_eq!(fr["name"], "get_trades");
        assert_eq!(fr["response"]["count"], 3);
        assert_eq!(body["contents"][0]["role"], "user");
    }

    #[test]
    fn renders_user_media_as_inline_data_part() {
        let messages = vec![LlmMessage {
            role: "user".to_owned(),
            content: None,
            tool_calls: None,
            tool_call_id: None,
            name: None,
            media: Some(vec![LlmMediaPart {
                mime_type: "image/png".to_owned(),
                data: Some("QUJD".to_owned()),
                file_uri: None,
            }]),
        }];
        let body = build_gemini_request(&messages, None, None, 0.2, 100);
        let parts = body["contents"][0]["parts"].as_array().unwrap();
        let inline = parts
            .iter()
            .find_map(|p| p.get("inline_data"))
            .expect("expected an inline_data part");
        assert_eq!(inline["mime_type"], "image/png");
        assert_eq!(inline["data"], "QUJD");
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

    #[test]
    fn classifies_plain_text_part() {
        let part = json!({ "text": "hello" });
        assert_eq!(
            classify_gemini_part(&part),
            GeminiPart::Text("hello".to_owned())
        );
    }

    #[test]
    fn classifies_thought_part_as_thought() {
        let part = json!({ "text": "thinking...", "thought": true });
        assert_eq!(
            classify_gemini_part(&part),
            GeminiPart::Thought("thinking...".to_owned())
        );
    }

    #[test]
    fn classifies_function_call_part() {
        let part =
            json!({ "functionCall": { "name": "get_trades", "args": { "symbol": "AAPL" } } });
        match classify_gemini_part(&part) {
            GeminiPart::FunctionCall { name, args, .. } => {
                assert_eq!(name, "get_trades");
                assert_eq!(args["symbol"], "AAPL");
            }
            other => panic!("expected FunctionCall, got {other:?}"),
        }
    }
}
