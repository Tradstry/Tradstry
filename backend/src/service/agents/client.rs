#![allow(dead_code)]

use anyhow::{Context, Result, anyhow};
use futures_util::StreamExt;
use rig::{client::CompletionClient, completion::Prompt, providers::groq};
use serde_json::Value;

use crate::service::chat::types::{
    ChatEventBus, ChatStreamEnvelope, ChatStreamKind, GroqChatResponse, GroqMessage, GroqToolDef,
};

const DEFAULT_GROQ_MODEL: &str = "openai/gpt-oss-120b";
const DEFAULT_GROQ_TEMPERATURE: f64 = 0.2;
const DEFAULT_GROQ_MAX_TOKENS: u64 = 300000;

#[derive(Clone, Debug)]
pub struct AgentsConfig {
    pub api_key: String,
    pub model: String,
    pub preamble: Option<String>,
    pub temperature: Option<f64>,
    pub max_tokens: Option<u64>,
}

impl AgentsConfig {
    pub fn from_env() -> Result<Self> {
        let api_key =
            std::env::var("GROQ_API_KEY").context("GROQ_API_KEY environment variable not set")?;
        let model = std::env::var("GROQ_MODEL")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_GROQ_MODEL.to_owned());
        let preamble = std::env::var("GROQ_PREAMBLE")
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty());

        Ok(Self {
            api_key,
            model,
            preamble,
            temperature: Some(DEFAULT_GROQ_TEMPERATURE),
            max_tokens: Some(DEFAULT_GROQ_MAX_TOKENS),
        })
    }
}

#[derive(Clone)]
pub struct AgentsClient {
    groq_client: groq::Client,
    config: AgentsConfig,
    http_client: reqwest::Client,
}

impl AgentsClient {
    pub fn new(config: AgentsConfig) -> Result<Self> {
        let groq_client =
            groq::Client::new(&config.api_key).context("Failed to create Groq client")?;

        let http_client = reqwest::Client::new();

        Ok(Self {
            groq_client,
            config,
            http_client,
        })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(AgentsConfig::from_env()?)
    }

    pub fn model(&self) -> &str {
        &self.config.model
    }

    pub fn preamble(&self) -> Option<&str> {
        self.config.preamble.as_deref()
    }

    pub async fn prompt(&self, prompt: impl AsRef<str>) -> Result<String> {
        let mut agent_builder = self.groq_client.agent(self.config.model.clone());

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

        agent
            .prompt(prompt.as_ref())
            .await
            .with_context(|| format!("Groq prompt failed for model {}", self.config.model))
    }

    pub async fn stream_chat(
        &self,
        messages: &[GroqMessage],
        tools: Option<&[GroqToolDef]>,
        tx: ChatEventBus,
        job_id: &str,
        session_id: &str,
    ) -> Result<GroqChatResponse> {
        // Build the request body
        let mut body = serde_json::json!({
            "model": self.config.model,
            "messages": messages,
            "stream": true,
            "temperature": 0.2,
        });

        if let Some(tools) = tools {
            body["tools"] = serde_json::to_value(tools)?;
        }

        // POST to Groq's OpenAI-compatible endpoint
        let response = self
            .http_client
            .post("https://api.groq.com/openai/v1/chat/completions")
            .header(
                "Authorization",
                format!("Bearer {}", self.config.api_key),
            )
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .context("Failed to send request to Groq API")?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Groq API error {}: {}", status, text));
        }

        // Accumulator state
        let mut full_text = String::new();
        let mut tool_call_id = String::new();
        let mut tool_call_name = String::new();
        let mut tool_call_arguments = String::new();
        let mut tool_name_sent = false;
        let mut is_tool_call = false;

        // Read the SSE byte stream
        let mut stream = response.bytes_stream();
        let mut buffer = String::new();

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error reading stream chunk")?;
            let text = String::from_utf8_lossy(&chunk);
            buffer.push_str(&text);

            // Process complete lines from the buffer
            loop {
                if let Some(newline_pos) = buffer.find('\n') {
                    let line = buffer[..newline_pos].trim_end_matches('\r').to_owned();
                    buffer = buffer[newline_pos + 1..].to_owned();

                    if line.is_empty() {
                        continue;
                    }

                    // Strip the "data: " prefix
                    let Some(data) = line.strip_prefix("data: ") else {
                        continue;
                    };

                    // Skip the stream terminator
                    if data == "[DONE]" {
                        break;
                    }

                    // Parse the JSON chunk
                    let chunk_val: Value = match serde_json::from_str(data) {
                        Ok(v) => v,
                        Err(_) => continue,
                    };

                    let delta = &chunk_val["choices"][0]["delta"];
                    if delta.is_null() {
                        continue;
                    }

                    // Handle content tokens
                    if let Some(content) = delta["content"].as_str() {
                        if !content.is_empty() {
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
                    }

                    // Handle tool call deltas
                    if let Some(tool_calls) = delta["tool_calls"].as_array() {
                        is_tool_call = true;
                        for tc in tool_calls {
                            // Accumulate tool call id (only appears in first delta)
                            if let Some(id) = tc["id"].as_str() {
                                if !id.is_empty() {
                                    tool_call_id = id.to_owned();
                                }
                            }

                            let func = &tc["function"];

                            // Accumulate name (comes once)
                            if let Some(name) = func["name"].as_str() {
                                if !name.is_empty() && tool_call_name.is_empty() {
                                    tool_call_name = name.to_owned();

                                    // Emit ToolStart as soon as we have the name
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
                            }

                            // Accumulate arguments (streams across many chunks)
                            if let Some(args) = func["arguments"].as_str() {
                                tool_call_arguments.push_str(args);
                            }
                        }
                    }
                } else {
                    // No more complete lines in buffer; wait for next chunk
                    break;
                }
            }
        }

        // Return the appropriate response variant
        if is_tool_call {
            Ok(GroqChatResponse::ToolCall {
                id: tool_call_id,
                name: tool_call_name,
                arguments: tool_call_arguments,
            })
        } else {
            Ok(GroqChatResponse::TextComplete { full_text })
        }
    }
}
