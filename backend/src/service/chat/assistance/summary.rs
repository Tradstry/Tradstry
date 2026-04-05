use anyhow::{Result, anyhow};

use crate::service::agents::client::AgentsClient;

pub async fn transform(agents: &AgentsClient, text: &str, action: &str) -> Result<String> {
    let prompt = match action {
        "summarize" => format!("Summarize this text concisely. Output ONLY the summary.\n\n{text}"),
        "fix_spelling" => format!(
            "Fix any spelling and grammar errors in this text. Output ONLY the corrected text, \
             preserving the original meaning and style.\n\n{text}"
        ),
        "simplify" => format!(
            "Rewrite this text in simpler, clearer language. Output ONLY the simplified text.\n\n{text}"
        ),
        "expand" => format!(
            "Expand this text with more detail and explanation. Output ONLY the expanded text.\n\n{text}"
        ),
        _ => return Err(anyhow!("Unknown action: {action}")),
    };

    let result = agents.prompt(&prompt).await?;
    Ok(result.trim().to_string())
}
