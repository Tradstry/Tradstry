//! LLM-generated context blurbs for RAG chunks. One Gemini call per (re)indexed
//! document produces a one-sentence situating blurb for every chunk, which the
//! indexer prepends to the embedded text. Results are cached by content hash.

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::service::ai::client::AgentsClient;

/// Master switch for the LLM context layer. Project convention keeps tuning
/// flags as code constants (not env vars). When `false`, the indexer falls back
/// to deterministic-only embedded text (Phase 2/3 behaviour).
pub const ENABLE_LLM_CONTEXT: bool = true;

#[derive(Deserialize)]
struct BlurbList {
    blurbs: Vec<String>,
}

/// One Gemini call: given the full document and its ordered chunks, return a
/// one-sentence situating blurb per chunk (same length/order as input). The
/// output is padded/truncated to exactly `chunks.len()` so callers can zip it
/// against the chunk list without bounds checks.
pub async fn generate_context_blurbs(
    agents: &AgentsClient,
    doc_title: &str,
    doc_body: &str,
    chunks: &[String],
) -> Result<Vec<String>> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }
    let chunk_list = chunks
        .iter()
        .enumerate()
        .map(|(i, c)| format!("CHUNK {i}:\n{c}"))
        .collect::<Vec<_>>()
        .join("\n\n");
    let prompt = format!(
        "You situate document chunks for retrieval. Document title: {doc_title}\n\n\
         Full document:\n{doc_body}\n\n\
         For EACH chunk below, write ONE short sentence (<=25 words) describing what the \
         chunk is about and how it fits the document. Return JSON only: \
         {{\"blurbs\": [\"...\", ...]}} with exactly {n} entries in order.\n\n{chunk_list}",
        n = chunks.len()
    );
    let raw = agents
        .prompt(&prompt)
        .await
        .context("gemini context blurbs failed")?;
    // Defensive JSON extraction: model output may be wrapped in prose/fences.
    let json_start = raw.find('{').unwrap_or(0);
    let parsed: BlurbList =
        serde_json::from_str(raw[json_start..].trim()).context("parse blurb JSON")?;
    let mut out = parsed.blurbs;
    out.resize(chunks.len(), String::new());
    Ok(out)
}
