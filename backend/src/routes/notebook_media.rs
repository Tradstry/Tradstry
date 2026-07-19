//! Hash-addressed notebook media. The client computes the SHA-256 of the bytes
//! before uploading; that hash is the identity everywhere (object key, DB
//! `content_hash`, the Lexical node's reference). Uploads are idempotent and the
//! bytes are deduplicated in R2 — the same image pasted into two notes stores one
//! object. Superssedes the id-addressed `/notebook/images` routes.

use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result, error, web};
use anyhow::{Context, anyhow, ensure};
use clerk_rs::validators::authorizer::ClerkJwt;
use futures_util::StreamExt;
use log::info;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::service::db::Db;
use crate::service::db::schema::tables::billing_table;
use crate::service::db::schema::tables::notebook::images::{
    CreateNotebookImageInput, NotebookImage,
};
use crate::service::r2::R2Client;
use crate::service::read_service::images as image_service;
use crate::service::read_service::notebook as notebook_service;
use crate::service::read_service::users::ensure_user;

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_VIDEO_BYTES: usize = 250 * 1024 * 1024;
const THUMB_MAX: u32 = 640;
// R2/SigV4 presigned URLs max out at 7 days.
const PRESIGN_TTL: Duration = Duration::from_secs(604_800);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadNotebookMediaResponse {
    image: NotebookImage,
}

#[derive(Deserialize)]
pub struct DeleteQuery {
    note_id: Option<String>,
}

/// The R2 object key for a user's content-addressed media. Two notes referencing
/// the same bytes resolve to the same key, so `put_object` is idempotent.
pub fn media_key(user_id: &str, hash: &str) -> String {
    format!("notebook/{user_id}/media/{hash}")
}

/// Reject bytes whose SHA-256 does not match the client-declared hash. Content
/// addressing only holds if the id is actually the digest of the bytes.
pub fn verify_hash(bytes: &[u8], expected_hex: &str) -> anyhow::Result<()> {
    let got = hex::encode(Sha256::digest(bytes));
    ensure!(
        got == expected_hex,
        "hash mismatch: computed {got}, client sent {expected_hex}"
    );
    Ok(())
}

async fn presign(r2: &R2Client, mut image: NotebookImage) -> NotebookImage {
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

/// Downscale an image to a JPEG thumbnail (longest side <= 640), or grab a video's
/// first keyframe via ffmpeg. Best-effort — a missing thumbnail is not fatal.
async fn make_thumb(media_type: &str, bytes: &[u8]) -> Option<Vec<u8>> {
    if media_type == "video" {
        return crate::service::media::extract_keyframes(bytes, 1)
            .await
            .into_iter()
            .next();
    }
    let img = image::load_from_memory(bytes).ok()?;
    let thumb = img.thumbnail(THUMB_MAX, THUMB_MAX);
    let mut out = std::io::Cursor::new(Vec::new());
    thumb.write_to(&mut out, image::ImageFormat::Jpeg).ok()?;
    Some(out.into_inner())
}

async fn read_media_payload(
    mut payload: Multipart,
) -> anyhow::Result<(String, String, String, Vec<u8>, String)> {
    let mut note_id: Option<String> = None;
    let mut hash: Option<String> = None;
    let mut filename = String::from("upload");
    let mut mime_type: Option<String> = None;
    let mut bytes = Vec::new();

    while let Some(field) = payload.next().await {
        let mut field =
            field.map_err(|error| anyhow!("Failed to read multipart field: {error}"))?;
        let field_name = field.name().unwrap_or_default().to_string();

        if field_name == "noteId" || field_name == "hash" {
            let mut value = Vec::new();
            while let Some(chunk) = field.next().await {
                let chunk =
                    chunk.map_err(|error| anyhow!("Failed to read {field_name} field: {error}"))?;
                value.extend_from_slice(&chunk);
            }
            let parsed = String::from_utf8(value).context("field must be utf-8")?;
            if field_name == "noteId" {
                note_id = Some(parsed.trim().to_string());
            } else {
                hash = Some(parsed.trim().to_string());
            }
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
    let hash = hash.ok_or_else(|| anyhow!("hash is required"))?;
    ensure!(!note_id.is_empty(), "noteId is required");
    ensure!(!hash.is_empty(), "hash is required");
    ensure!(!bytes.is_empty(), "file is required");

    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());
    ensure!(
        mime_type.starts_with("image/") || mime_type.starts_with("video/"),
        "Only image and video uploads are supported"
    );

    Ok((note_id, hash, filename, bytes, mime_type))
}

pub async fn upload_notebook_media(
    req: HttpRequest,
    payload: Multipart,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
    redis: web::Data<Option<Arc<crate::service::redis::client::RedisClient>>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let (note_id, hash, filename, bytes, mime_type) = read_media_payload(payload)
        .await
        .map_err(error::ErrorBadRequest)?;

    verify_hash(&bytes, &hash).map_err(error::ErrorBadRequest)?;

    let note = notebook_service::get_notebook_note(&user_db, &note_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook note not found"))?;

    // Idempotent: re-uploading the same bytes into the same note returns the row.
    if let Some(existing) =
        image_service::find_notebook_image_for_note_hash(&user_db, &note.id, &hash)
            .await
            .map_err(error::ErrorInternalServerError)?
    {
        let existing = presign(r2.get_ref(), existing).await;
        return Ok(HttpResponse::Ok().json(UploadNotebookMediaResponse { image: existing }));
    }

    let media_type = if mime_type.starts_with("video/") {
        "video"
    } else {
        "image"
    };
    let format = mime_type.rsplit('/').next().unwrap_or("").to_string();
    let byte_len = bytes.len() as i64;

    let (width, height, duration_seconds) = if media_type == "video" {
        let meta = crate::service::media::probe_video(&bytes).await;
        (meta.width, meta.height, meta.duration_seconds)
    } else {
        match imagesize::blob_size(&bytes) {
            Ok(dim) => (dim.width as i64, dim.height as i64, 0.0),
            Err(_) => (0, 0, 0.0),
        }
    };

    let object_key = media_key(user_db.user_id(), &hash);
    info!(
        "Uploading notebook media: user_id={} note_id={} hash={} type={}",
        user_db.user_id(),
        note.id,
        hash,
        media_type
    );

    // Only put (and arm cleanup) if the bytes are not already stored — content
    // addressing means an existing object is the identical bytes, shared by other
    // notes; deleting it on our orphan-cleanup would corrupt those references.
    let already_stored = r2
        .object_exists(&object_key)
        .await
        .map_err(error::ErrorInternalServerError)?;

    let mut cleanup = if already_stored {
        // Content-addressed: these exact bytes are already billed to this user.
        None
    } else {
        // Refuse before the bytes reach R2, so an over-cap upload costs nothing.
        // The body carries the structured code so the editor can offer an
        // upgrade rather than reporting a generic upload failure.
        if let Err(e) = crate::service::billing::quota::check_media_headroom(
            db.pool(),
            redis.get_ref().as_deref(),
            user_db.user_id(),
            byte_len,
        )
        .await
        {
            return Ok(HttpResponse::Forbidden().json(e.to_json()));
        }

        let guard = R2UploadGuard::new(r2.get_ref().clone(), object_key.clone());
        r2.put_object(&object_key, bytes.clone(), &mime_type)
            .await
            .map_err(error::ErrorInternalServerError)?;
        if let Some(thumb) = make_thumb(media_type, &bytes).await {
            let _ = r2
                .put_object(&format!("{object_key}.thumb"), thumb, "image/jpeg")
                .await;
        }
        Some(guard)
    };

    let image = image_service::create_notebook_image(
        &user_db,
        CreateNotebookImageInput {
            id: Uuid::new_v4().to_string(),
            note_id: note.id,
            account_id: note.account_id,
            cloudinary_asset_id: hash.clone(),
            cloudinary_public_id: object_key,
            secure_url: String::new(),
            width,
            height,
            format,
            bytes: byte_len,
            original_filename: filename,
            media_type: media_type.to_string(),
            content_type: mime_type,
            duration_seconds,
            content_hash: hash,
        },
    )
    .await
    .map_err(error::ErrorInternalServerError)?;

    if let Some(guard) = cleanup.as_mut() {
        guard.disarm();
    }

    // Only new bytes count. A second note referencing the same object shares it.
    if !already_stored {
        if let Err(e) = billing_table::add_media_bytes(db.pool(), user_db.user_id(), byte_len).await
        {
            // The upload succeeded; a lost increment self-corrects on the next
            // usage recompute, so this must not fail the request.
            log::warn!("Failed to add media bytes for {}: {e:#}", user_db.user_id());
        }
        crate::service::billing::entitlements::invalidate(
            redis.get_ref().as_deref(),
            user_db.user_id(),
        )
        .await;
    }

    let image = presign(r2.get_ref(), image).await;
    Ok(HttpResponse::Ok().json(UploadNotebookMediaResponse { image }))
}

pub async fn get_notebook_media(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let hash = path.into_inner();

    let image = image_service::find_notebook_image_by_hash(&user_db, &hash)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook media not found"))?;

    let bytes = r2
        .get_object(&image.cloudinary_public_id)
        .await
        .map_err(error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok()
        .content_type(image.content_type)
        .body(bytes))
}

pub async fn get_notebook_media_thumb(
    req: HttpRequest,
    path: web::Path<String>,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let hash = path.into_inner();

    let image = image_service::find_notebook_image_by_hash(&user_db, &hash)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook media not found"))?;

    let bytes = r2
        .get_object(&format!("{}.thumb", image.cloudinary_public_id))
        .await
        .map_err(|_| error::ErrorNotFound("Thumbnail not found"))?;

    Ok(HttpResponse::Ok().content_type("image/jpeg").body(bytes))
}

pub async fn delete_notebook_media(
    req: HttpRequest,
    path: web::Path<String>,
    query: web::Query<DeleteQuery>,
    db: web::Data<Arc<Db>>,
    r2: web::Data<Arc<R2Client>>,
    redis: web::Data<Option<Arc<crate::service::redis::client::RedisClient>>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, db.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let hash = path.into_inner();
    let note_id = query
        .into_inner()
        .note_id
        .ok_or_else(|| error::ErrorBadRequest("noteId query parameter is required"))?;

    if let Some(image) = image_service::find_notebook_image_for_note_hash(&user_db, &note_id, &hash)
        .await
        .map_err(error::ErrorInternalServerError)?
    {
        image_service::delete_notebook_image(&user_db, &image.id)
            .await
            .map_err(error::ErrorInternalServerError)?;

        // Refcount: only remove the shared bytes once no note references them.
        let remaining = image_service::count_notebook_images_with_hash(&user_db, &hash)
            .await
            .map_err(error::ErrorInternalServerError)?;
        if remaining == 0 {
            let _ = r2.delete_object(&image.cloudinary_public_id).await;
            let _ = r2
                .delete_object(&format!("{}.thumb", image.cloudinary_public_id))
                .await;

            // Symmetric with upload: the quota is only released once the bytes
            // actually go, not when one of several references does.
            if let Err(e) =
                billing_table::add_media_bytes(db.pool(), user_db.user_id(), -image.bytes).await
            {
                log::warn!(
                    "Failed to release media bytes for {}: {e:#}",
                    user_db.user_id()
                );
            }
            crate::service::billing::entitlements::invalidate(
                redis.get_ref().as_deref(),
                user_db.user_id(),
            )
            .await;
        }
    }

    Ok(HttpResponse::NoContent().finish())
}

/// Deletes a just-written R2 object on drop unless `disarm()` is called first.
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
            tokio::spawn(async move {
                if let Err(error) = r2.delete_object(&key).await {
                    log::warn!("Failed to clean up orphaned R2 upload {key}: {error}");
                }
                let _ = r2.delete_object(&format!("{key}.thumb")).await;
            });
        }
    }
}
