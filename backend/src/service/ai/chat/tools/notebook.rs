use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::vector_database::blocks::extract_notebook_blocks;
use crate::service::read_service::notebook as notebook_service;
use crate::service::turso::TursoClient;

#[derive(Debug, Default, Deserialize)]
struct GetNotebookInput {
    note_id: Option<String>,
    account_id: Option<String>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "get_notebook".to_string(),
            description:
                "List the user's notebook notes with their text content and a manifest of \
                 attached media (images/videos) — including each media_id. Pass note_id for one \
                 note, or account_id to scope to an account. To actually view an image or analyze \
                 a video, call view_media with a media_id from this manifest."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "note_id": { "type": "string", "description": "Optional single note id." },
                    "account_id": { "type": "string", "description": "Optional account id to scope the listing." }
                }
            }),
        },
    }
}

pub async fn execute(arguments: &str, user_id: &str, turso: &Arc<TursoClient>) -> Result<String> {
    let input: GetNotebookInput = serde_json::from_str(arguments).unwrap_or_default();
    let user_db = turso.get_user_db(user_id).await?;

    let notes = match &input.note_id {
        Some(id) => notebook_service::get_notebook_note(&user_db, id)
            .await?
            .into_iter()
            .collect::<Vec<_>>(),
        None => {
            notebook_service::list_notebook_notes(&user_db, input.account_id.as_deref()).await?
        }
    };

    let out: Vec<serde_json::Value> = notes
        .iter()
        .map(|n| {
            let text = extract_notebook_blocks(&n.document_json)
                .into_iter()
                .map(|b| b.text)
                .collect::<Vec<_>>()
                .join("\n");
            let media: Vec<serde_json::Value> = n
                .images
                .iter()
                .map(|m| {
                    json!({
                        "media_id": m.id,
                        "media_type": m.media_type,
                        "content_type": m.content_type,
                        "width": m.width,
                        "height": m.height,
                        "duration_seconds": m.duration_seconds,
                    })
                })
                .collect();
            json!({
                "id": n.id,
                "title": n.title,
                "account_id": n.account_id,
                "folder_id": n.folder_id,
                "text": text,
                "media": media,
            })
        })
        .collect();

    Ok(serde_json::to_string(&out)?)
}
