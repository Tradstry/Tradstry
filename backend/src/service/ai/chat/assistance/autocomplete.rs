use anyhow::Result;

use crate::service::ai::client::AgentsClient;

pub async fn complete(agents: &AgentsClient, text: &str, cursor_offset: usize) -> Result<String> {
    let before_cursor = if cursor_offset <= text.len() {
        &text[..text.floor_char_boundary(cursor_offset)]
    } else {
        text
    };

    if before_cursor.trim().is_empty() {
        return Ok(String::new());
    }

    let prompt = format!(
        "Continue this text naturally with 5-15 words. Output ONLY the continuation, nothing else. \
         No quotes, no explanation, no repeating the input.\n\n{before_cursor}"
    );

    let result = agents.prompt(&prompt).await?;

    // Preserve word boundary: if the input ends mid-word (no trailing space)
    // and the completion doesn't start with a space, prepend one.
    let trimmed = result.trim_end().to_string();
    let needs_space = !before_cursor.ends_with(' ')
        && !before_cursor.ends_with('\n')
        && !trimmed.starts_with(' ')
        && !trimmed.starts_with(',')
        && !trimmed.starts_with('.')
        && !trimmed.starts_with('!')
        && !trimmed.starts_with('?')
        && !trimmed.starts_with(';')
        && !trimmed.starts_with(':');

    if needs_space {
        Ok(format!(" {trimmed}"))
    } else {
        Ok(trimmed)
    }
}
