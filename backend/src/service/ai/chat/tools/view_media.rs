use anyhow::Result;
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::ai::client::AgentsClient;
use crate::service::db::Db;
use crate::service::r2::R2Client;
use crate::service::read_service::notebook as notebook_service;

#[derive(Debug, Default, Deserialize)]
struct ViewMediaInput {
    media_id: String,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "view_media".to_string(),
            description:
                "View a single notebook media item (image or video) by its media_id so you can \
                 actually SEE it. Pass a media_id obtained from get_notebook's media manifest. \
                 For an image, the image is loaded into the conversation for you to analyze on \
                 the next turn."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "media_id": {
                        "type": "string",
                        "description": "The media_id of the image/video to view, from get_notebook."
                    }
                },
                "required": ["media_id"]
            }),
        },
    }
}

/// Returns a JSON media-envelope STRING:
/// `{ "text": "...", "media": [{ "mime_type": "...", "data": "<base64>" }] }`.
/// The graph tool node parses this for `view_media` and injects the media as a
/// follow-up user turn so Gemini sees the image.
pub async fn execute(
    arguments: &str,
    user_id: &str,
    r2: &Arc<R2Client>,
    db: &Arc<Db>,
    agents: &Arc<AgentsClient>,
) -> Result<String> {
    let input: ViewMediaInput = serde_json::from_str(arguments).unwrap_or_default();
    if input.media_id.is_empty() {
        return Ok(json!({
            "text": "No media_id provided. Call get_notebook first to find a media_id.",
            "media": []
        })
        .to_string());
    }

    let user_db = db.get_user_db(user_id);

    let media = match notebook_service::find_notebook_image(&user_db, &input.media_id).await? {
        Some(m) => m,
        None => {
            return Ok(json!({
                "text": format!("Media {} not found.", input.media_id),
                "media": []
            })
            .to_string());
        }
    };

    // Video is delivered via the Gemini Files API: upload the raw bytes, get a
    // file_uri, and return it as a file_data media part for build_gemini_request.
    if media.media_type == "video" {
        let bytes = r2.get_object(&media.cloudinary_public_id).await?;
        let uri = agents.upload_file(bytes, &media.content_type).await?;
        return Ok(json!({
            "text": format!("Uploaded video {} for analysis.", input.media_id),
            "media": [{ "mime_type": media.content_type, "file_uri": uri }]
        })
        .to_string());
    }

    let bytes = r2.get_object(&media.cloudinary_public_id).await?;
    let b64 = BASE64.encode(&bytes);

    Ok(json!({
        "text": format!("Loaded image {} ({}).", input.media_id, media.content_type),
        "media": [{ "mime_type": media.content_type, "data": b64 }]
    })
    .to_string())
}
