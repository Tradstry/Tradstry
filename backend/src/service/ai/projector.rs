use anyhow::{Context, Result, anyhow};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

// Asymmetric stdio contract: `project` takes base64 update blobs (one per line) and
// emits document JSON; `seed` takes raw document JSON and emits a single base64 blob.
const PROJECT_ENTRY: &str = "project.mjs";
const SEED_ENTRY: &str = "seed.mjs";
const COMPACT_ENTRY: &str = "compact.mjs";
const MARKDOWN_ENTRY: &str = "markdown.mjs";

/// Where `projector/` lives. In the Docker image it sits next to the binary; under
/// `cargo test` the binary is in target/debug, so fall back to the crate root.
fn projector_dir() -> std::path::PathBuf {
    if let Ok(dir) = std::env::var("PROJECTOR_DIR") {
        return std::path::PathBuf::from(dir);
    }
    if let Ok(exe) = std::env::current_exe()
        && let Some(beside_binary) = exe.parent().map(|p| p.join("projector"))
        && beside_binary.is_dir()
    {
        return beside_binary;
    }
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/projector"))
}

async fn run(script: &str, stdin: Vec<u8>) -> Result<Vec<u8>> {
    let entry = projector_dir().join(script);
    let entry = entry.display().to_string();
    let entry = entry.as_str();
    // Lexical's dev bundles have a circular init that throws under Bun ("Cannot access
    // 'defineImportRule' before initialization"), which fails every seed and freezes the
    // note in `seeding` forever. Bun does not infer conditions from NODE_ENV, so the
    // production exports must be selected explicitly.
    let mut child = Command::new("bun")
        .arg("--conditions=production")
        .arg(entry)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn bun for {entry}"))?;

    child
        .stdin
        .take()
        .context("child stdin was not piped")?
        .write_all(&stdin)
        .await
        .context("failed to write to bun stdin")?;

    let output = child
        .wait_with_output()
        .await
        .context("failed to wait on bun")?;

    // Never yield an empty document on failure: a blank projection would silently wipe
    // the note's derived title and its vector-search embeddings.
    if !output.status.success() || output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!(
            "projector {entry} failed (status {}): {}",
            output.status,
            stderr.trim()
        ));
    }

    Ok(output.stdout)
}

pub async fn project(updates: &[Vec<u8>]) -> Result<String> {
    // Projecting zero updates is a bug, not an empty document; fail before spawning.
    if updates.is_empty() {
        return Err(anyhow!("project: refusing to project zero updates"));
    }

    let mut stdin = Vec::new();
    for update in updates {
        stdin.extend_from_slice(BASE64.encode(update).as_bytes());
        stdin.push(b'\n');
    }

    let stdout = run(PROJECT_ENTRY, stdin).await?;
    String::from_utf8(stdout).context("projector emitted non-UTF-8 document JSON")
}

pub struct Seeded {
    pub update: Vec<u8>,
    /// A real Yjs state vector for the seeded doc; delta-pull work depends on it.
    pub state_vector: Vec<u8>,
}

pub async fn seed(document_json: &str) -> Result<Seeded> {
    #[derive(serde::Deserialize)]
    struct Raw {
        update: String,
        #[serde(rename = "stateVector")]
        state_vector: String,
    }

    let stdout = run(SEED_ENTRY, document_json.as_bytes().to_vec()).await?;
    let raw: Raw = serde_json::from_slice(&stdout).context("seeder emitted invalid JSON")?;
    Ok(Seeded {
        update: BASE64
            .decode(raw.update.trim())
            .context("seeder emitted invalid base64 update")?,
        state_vector: BASE64
            .decode(raw.state_vector.trim())
            .context("seeder emitted invalid base64 state vector")?,
    })
}

/// Collapses a note's update chain into one equivalent blob.
pub async fn compact(updates: &[Vec<u8>]) -> Result<Vec<u8>> {
    // An empty merge yields a blob that projects as an empty document, which would
    // replace the note's whole history with nothing.
    if updates.is_empty() {
        return Err(anyhow!("compact: refusing to compact zero updates"));
    }

    let mut stdin = Vec::new();
    for update in updates {
        stdin.extend_from_slice(BASE64.encode(update).as_bytes());
        stdin.push(b'\n');
    }

    let stdout = run(COMPACT_ENTRY, stdin).await?;
    let encoded = String::from_utf8(stdout).context("compactor emitted non-UTF-8 output")?;
    BASE64
        .decode(encoded.trim())
        .context("compactor emitted invalid base64 update")
}

/// How an agent-supplied markdown body is folded into an existing note.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditMode {
    Replace,
    Append,
}

impl EditMode {
    fn as_str(self) -> &'static str {
        match self {
            EditMode::Replace => "replace",
            EditMode::Append => "append",
        }
    }
}

/// Markdown -> Lexical `document_json`, for a note that does not exist yet.
///
/// The conversion lives in the projector rather than Rust so there is exactly one
/// implementation of "what Tradstry understands" — the same bundle that seeds and projects.
pub async fn markdown_to_json(markdown: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Raw {
        #[serde(rename = "documentJson")]
        document_json: String,
    }

    let stdin = serde_json::json!({ "op": "toJson", "markdown": markdown });
    let stdout = run(MARKDOWN_ENTRY, serde_json::to_vec(&stdin)?).await?;
    let raw: Raw = serde_json::from_slice(&stdout).context("markdown emitted invalid JSON")?;
    Ok(raw.document_json)
}

/// Appends markdown to a not-yet-CRDT note's `document_json`.
///
/// Structural rather than a markdown round-trip: rendering the body to markdown and back
/// would drop every node markdown cannot express (images, linked trades).
pub async fn append_markdown_to_json(document_json: &str, markdown: &str) -> Result<String> {
    #[derive(serde::Deserialize)]
    struct Raw {
        #[serde(rename = "documentJson")]
        document_json: String,
    }

    let stdin = serde_json::json!({
        "op": "appendJson",
        "documentJson": document_json,
        "markdown": markdown,
    });
    let stdout = run(MARKDOWN_ENTRY, serde_json::to_vec(&stdin)?).await?;
    let raw: Raw = serde_json::from_slice(&stdout).context("markdown emitted invalid JSON")?;
    Ok(raw.document_json)
}

/// Edits a CRDT note, returning ONLY the incremental Yjs update to append.
///
/// A note whose body is CRDT-backed cannot be edited by rewriting `document_json` — the
/// clients rebuild content from the update chain and would never see the change.
pub async fn apply_markdown(
    updates: &[Vec<u8>],
    markdown: &str,
    mode: EditMode,
) -> Result<Vec<u8>> {
    if updates.is_empty() {
        return Err(anyhow!("apply_markdown: note has no updates to edit"));
    }

    #[derive(serde::Deserialize)]
    struct Raw {
        update: String,
    }

    let encoded: Vec<String> = updates.iter().map(|u| BASE64.encode(u)).collect();
    let stdin = serde_json::json!({
        "op": "apply",
        "markdown": markdown,
        "mode": mode.as_str(),
        "updates": encoded,
    });

    let stdout = run(MARKDOWN_ENTRY, serde_json::to_vec(&stdin)?).await?;
    let raw: Raw = serde_json::from_slice(&stdout).context("markdown emitted invalid JSON")?;
    BASE64
        .decode(raw.update.trim())
        .context("markdown emitted invalid base64 update")
}
