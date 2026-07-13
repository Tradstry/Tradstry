use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result, error, web};
use anyhow::{Context, anyhow, ensure};
use clerk_rs::validators::authorizer::ClerkJwt;
use futures_util::StreamExt;
use log::info;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::service::db::Db;
use crate::service::db::schema::tables::notebook::images::{
    CreateNotebookImageInput, NotebookImage,
};
use crate::service::r2::R2Client;
use crate::service::read_service::images as image_service;
use crate::service::read_service::notebook as notebook_service;
use crate::service::read_service::users::ensure_user;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_VIDEO_BYTES: usize = 250 * 1024 * 1024;
// R2/SigV4 presigned URLs max out at 7 days.
const PRESIGN_TTL: Duration = Duration::from_secs(604_800);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadNotebookImageResponse {
    image: NotebookImage,
}

/// Deletes a just-written R2 object on drop unless `disarm()` is called first.
///
/// Makes the upload cancellation-safe. actix-web drops the in-flight handler
/// future when the client disconnects — e.g. the user cancels an upload after
/// the bytes have already reached the server. Without this, the object we wrote
/// to R2 (and a failed DB insert) would be orphaned. The guard fires on that
/// drop, on any early `?` return, and only stands down once the DB row is
/// committed.
struct R2UploadGuard {
    r2: Arc<R2Client>,
    object_key: Option<String>,
}

impl R2UploadGuard {
    fn new(r2: Arc<R2Client>, object_key: String) -> Self {
        Self {
            r2,
            object_key: Some(object_key),
        }
    }

    fn disarm(&mut self) {
        self.object_key = None;
    }
}

impl Drop for R2UploadGuard {
    fn drop(&mut self) {
        if let Some(key) = self.object_key.take() {
            let r2 = self.r2.clone();
            // Drop is synchronous, so detach the async delete onto the runtime.
            tokio::spawn(async move {
                if let Err(error) = r2.delete_object(&key).await {
                    log::warn!("Failed to clean up orphaned R2 upload {key}: {error}");
                }
            });
        }
    }
}

/// Overwrite the stored (empty) `secure_url` with a freshly presigned R2 GET
/// URL derived from the object key. Falls back to leaving it empty on failure.
async fn presign(r2: &R2Client, mut image: NotebookImage) -> NotebookImage {
    // Only R2-backed rows store an empty secure_url; rows still on Cloudinary
    // keep their existing URL until migrated (dual-read safety).
    if image.secure_url.is_empty()
        && let Ok(url) = r2
            .presigned_get_url(&image.cloudinary_public_id, PRESIGN_TTL)
            .await
    {
        image.secure_url = url;
    }
    image
}

async fn get_user_db(
    req: &HttpRequest,
    db: &Arc<Db>,
) -> anyhow::Result<crate::service::db::client::UserDb> {
    let jwt = req
        .extensions()
        .get::<ClerkJwt>()
        .cloned()
        .ok_or_else(|| anyhow!("Unauthorized"))?;
    let pool = db.pool();

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let user = ensure_user(pool, &jwt.sub, full_name, email).await?;

    Ok(db.get_user_db(&user.id))
}

async fn read_upload_payload(
    mut payload: Multipart,
) -> anyhow::Result<(String, String, Vec<u8>, String)> {
    let mut note_id: Option<String> = None;
    let mut filename = String::from("upload");
    let mut mime_type: Option<String> = None;
    let mut bytes = Vec::new();

    while let Some(field) = payload.next().await {
        let mut field =
            field.map_err(|error| anyhow!("Failed to read multipart field: {error}"))?;
        let field_name = field.name().unwrap_or_default().to_string();

        if field_name == "noteId" {
            let mut value = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk =
                    chunk.map_err(|error| anyhow!("Failed to read noteId field: {error}"))?;
                value.extend_from_slice(&chunk);
            }

            let parsed = String::from_utf8(value).context("noteId must be utf-8")?;
            note_id = Some(parsed.trim().to_string());
            continue;
        }

        if field_name != "file" {
            while let Some(chunk) = field.next().await {
                chunk.map_err(|error| anyhow!("Failed to discard multipart field: {error}"))?;
            }
            continue;
        }

        if let Some(content_type) = field.content_type().cloned() {
            mime_type = Some(content_type.essence_str().to_string());
        }

        if let Some(content_disposition) = field.content_disposition()
            && let Some(original_name) = content_disposition.get_filename()
            && !original_name.trim().is_empty()
        {
            filename = original_name.trim().to_string();
        }

        // Pick the size limit from the declared type (images are small, videos
        // can be large). Unknown types fall back to the image limit.
        let resolved_mime = mime_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());
        let max_bytes = if resolved_mime.starts_with("video/") {
            MAX_VIDEO_BYTES
        } else {
            MAX_IMAGE_BYTES
        };

        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(|error| anyhow!("Failed to read uploaded bytes: {error}"))?;
            ensure!(
                bytes.len() + chunk.len() <= max_bytes,
                "File exceeds the {}MB upload limit",
                max_bytes / (1024 * 1024)
            );
            bytes.extend_from_slice(&chunk);
        }
    }

    let note_id = note_id.ok_or_else(|| anyhow!("noteId is required"))?;
    ensure!(!note_id.is_empty(), "noteId is required");
    ensure!(!bytes.is_empty(), "file is required");

    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());
    ensure!(
        mime_type.starts_with("image/") || mime_type.starts_with("video/"),
        "Only image and video uploads are supported"
    );

    Ok((note_id, filename, bytes, mime_type))
}

pub async fn upload_notebook_image(
    req: HttpRequest,
    payload: Multipart,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let (note_id, filename, bytes, mime_type) = read_upload_payload(payload)
        .await
        .map_err(error::ErrorBadRequest)?;

    let note = notebook_service::get_notebook_note(&user_db, &note_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook note not found"))?;

    let media_type = if mime_type.starts_with("video/") {
        "video"
    } else {
        "image"
    };
    let format = mime_type.rsplit('/').next().unwrap_or("").to_string();
    let byte_len = bytes.len() as i64;

    // Probe dimensions (and duration for video). Images: read header dims.
    // Video: ffprobe for width/height/duration (best-effort, never blocks upload).
    let (width, height, duration_seconds) = if media_type == "video" {
        let meta = crate::service::media::probe_video(&bytes).await;
        (meta.width, meta.height, meta.duration_seconds)
    } else {
        match imagesize::blob_size(&bytes) {
            Ok(dim) => (dim.width as i64, dim.height as i64, 0.0),
            Err(_) => (0, 0, 0.0),
        }
    };

    let media_id = Uuid::new_v4().to_string();
    let object_key = format!(
        "notebook/{}/{}/{}",
        user_db.user_id(),
        note.account_id,
        media_id
    );

    info!(
        "Uploading notebook media: user_id={} account_id={} note_id={} media_id={} type={}",
        user_db.user_id(),
        note.account_id,
        note.id,
        media_id,
        media_type
    );

    // Arm cleanup before the object exists so a disconnect/error anywhere from
    // here until the DB row is committed deletes the orphaned R2 object.
    let mut cleanup = R2UploadGuard::new(r2.get_ref().clone(), object_key.clone());

    r2.put_object(&object_key, bytes, &mime_type)
        .await
        .map_err(error::ErrorInternalServerError)?;

    let image = image_service::create_notebook_image(
        &user_db,
        CreateNotebookImageInput {
            id: media_id.clone(),
            note_id: note.id,
            account_id: note.account_id,
            // The legacy "cloudinary" columns are reused: asset_id holds the
            // media id (unique), public_id holds the R2 object key (unique).
            cloudinary_asset_id: media_id,
            cloudinary_public_id: object_key,
            // Not the serving URL anymore — presigned at read time.
            secure_url: String::new(),
            width,
            height,
            format,
            bytes: byte_len,
            original_filename: filename,
            media_type: media_type.to_string(),
            content_type: mime_type,
            duration_seconds,
            // Legacy path predates content addressing; the hash-addressed
            // /notebook/media route supersedes it.
            content_hash: String::new(),
        },
    )
    .await
    .map_err(error::ErrorInternalServerError)?;

    // Committed: the object now has a referencing row, so keep it.
    cleanup.disarm();

    let image = presign(r2.get_ref(), image).await;
    Ok(HttpResponse::Ok().json(UploadNotebookImageResponse { image }))
}

pub async fn get_notebook_image(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let image_id = path.into_inner();

    let image = image_service::get_notebook_image(&user_db, &image_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook media not found"))?;

    let image = presign(r2.get_ref(), image).await;
    Ok(HttpResponse::Ok().json(image))
}

pub async fn delete_notebook_image(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let image_id = path.into_inner();

    let image = image_service::get_notebook_image(&user_db, &image_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook media not found"))?;

    r2.delete_object(&image.cloudinary_public_id)
        .await
        .map_err(error::ErrorInternalServerError)?;

    image_service::delete_notebook_image(&user_db, &image.id)
        .await
        .map_err(error::ErrorInternalServerError)?;

    Ok(HttpResponse::NoContent().finish())
}
