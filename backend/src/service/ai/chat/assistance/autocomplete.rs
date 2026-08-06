use anyhow::Result;

use crate::service::ai::client::AgentsClient;

/// A terse, domain-anchored persona that keeps the model finishing the author's
/// own sentence instead of drifting into generic prose.
const COMPLETION_PREAMBLE: &str = "You are an inline autocomplete inside a trader's private trading journal. \
Continue the author's current sentence in THEIR voice — same tense, person, and register.\n\
Rules:\n\
- Output ONLY the continuation. No quotes, no labels, no explanation.\n\
- Finish the current thought in about 3 to 8 words. Never start a new sentence, paragraph, or topic.\n\
- Never repeat or restate words the author already wrote.\n\
- Match their style: terse fragments stay terse; full sentences get completed.\n\
- Entries discuss setups, entries and exits, risk, P&L, emotions, and lessons — stay on that subject.\n\
- If you cannot confidently continue, output nothing.\n\
\n\
Example:\n\
Text so far: \"I trimmed half at resistance because\"\n\
Continuation: the volume was drying up and I didn't want to give it back.";

/// A few words, not a paragraph. Output length is also bounded by the prompt.
const MAX_TOKENS: u64 = 64;
/// Hard word cap on the returned continuation, as a backstop to the prompt.
const MAX_WORDS: usize = 20;

/// `preceding` is the note text up to the caret (already windowed by the client);
/// `title` anchors the topic. Returns the continuation to show as ghost text, or
/// an empty string when there is nothing confident to suggest.
pub async fn complete(agents: &AgentsClient, title: &str, preceding: &str) -> Result<String> {
    if preceding.trim().is_empty() {
        return Ok(String::new());
    }

    let title = title.trim();
    let title_line = if title.is_empty() {
        String::new()
    } else {
        format!("Note title: {title}\n\n")
    };
    let prompt = format!("{title_line}Text so far:\n{preceding}\n\nContinuation:");

    let result = agents
        .prompt_with(COMPLETION_PREAMBLE, MAX_TOKENS, &prompt)
        .await?;

    Ok(clean_completion(&result, preceding))
}

/// Strips the ways the model can go off-track: leading labels, wrapping quotes,
/// spilling onto a new line, echoing the tail the author just typed, or running
/// long — then restores the word boundary the raw text lost.
fn clean_completion(raw: &str, preceding: &str) -> String {
    // The model must not open a new paragraph — keep only the first line.
    let mut c = raw
        .trim_start_matches(['\n', ' '])
        .lines()
        .next()
        .unwrap_or("")
        .trim()
        .to_string();

    for label in ["Continuation:", "continuation:"] {
        if let Some(rest) = c.strip_prefix(label) {
            c = rest.trim().to_string();
        }
    }

    if c.len() >= 2 && c.starts_with('"') && c.ends_with('"') {
        c = c[1..c.len() - 1].trim().to_string();
    }

    if c.is_empty() {
        return String::new();
    }

    // A completion that merely repeats the tail the author already wrote is noise.
    let tail_len = preceding.chars().count().saturating_sub(80);
    let tail: String = preceding.chars().skip(tail_len).collect();
    if c.chars().count() > 3 && tail.to_lowercase().contains(&c.to_lowercase()) {
        return String::new();
    }

    let words: Vec<&str> = c.split_whitespace().collect();
    if words.len() > MAX_WORDS {
        c = words[..MAX_WORDS].join(" ");
    }

    // The author's text ended on a whole word with no trailing space, so the
    // suggestion needs a leading space unless it opens with punctuation.
    let needs_space = !preceding.ends_with(' ')
        && !preceding.ends_with('\n')
        && !c.starts_with(|ch: char| " ,.!?;:".contains(ch));
    if needs_space { format!(" {c}") } else { c }
}
