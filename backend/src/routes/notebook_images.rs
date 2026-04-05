use actix_multipart::Multipart;
use actix_web::{HttpMessage, HttpRequest, HttpResponse, Result, error, web};
use anyhow::{Context, anyhow, ensure};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use clerk_rs::validators::authorizer::ClerkJwt;
use futures_util::StreamExt;
use log::info;
use serde::Serialize;
use std::sync::Arc;
use uuid::Uuid;

use crate::service::cloudinary::CloudinaryClient;
use crate::service::read_service::images as image_service;
use crate::service::read_service::notebook as notebook_service;
use crate::service::read_service::users::ensure_user;
use crate::service::turso::TursoClient;
use crate::service::turso::schema::tables::notebook_images::{
    CreateNotebookImageInput, NotebookImage,
};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UploadNotebookImageResponse {
    image: NotebookImage,
}

async fn get_user_db(
    req: &HttpRequest,
    turso: &Arc<TursoClient>,
) -> anyhow::Result<crate::service::turso::client::UserDb> {
    let jwt = req
        .extensions()
        .get::<ClerkJwt>()
        .cloned()
        .ok_or_else(|| anyhow!("Unauthorized"))?;
    let conn = turso.get_connection()?;

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

    let user = ensure_user(&conn, &jwt.sub, full_name, email).await?;

    turso.get_user_db(&user.id).await
}

async fn read_upload_payload(
    mut payload: Multipart,
) -> anyhow::Result<(String, String, Vec<u8>, String)> {
    let mut note_id: Option<String> = None;
    let mut filename = String::from("image");
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

        while let Some(chunk) = field.next().await {
            let chunk =
                chunk.map_err(|error| anyhow!("Failed to read uploaded image bytes: {error}"))?;
            ensure!(
                bytes.len() + chunk.len() <= MAX_IMAGE_BYTES,
                "Image exceeds the 10MB upload limit"
            );
            bytes.extend_from_slice(&chunk);
        }
    }

    let note_id = note_id.ok_or_else(|| anyhow!("noteId is required"))?;
    ensure!(!note_id.is_empty(), "noteId is required");
    ensure!(!bytes.is_empty(), "file is required");

    let mime_type = mime_type.unwrap_or_else(|| "application/octet-stream".to_string());
    ensure!(
        mime_type.starts_with("image/"),
        "Only image uploads are supported"
    );

    Ok((note_id, filename, bytes, mime_type))
}

pub async fn upload_notebook_image(
    req: HttpRequest,
    payload: Multipart,
    turso: web::Data<Arc<TursoClient>>,
    cloudinary: web::Data<Arc<CloudinaryClient>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, turso.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let (note_id, filename, bytes, mime_type) = read_upload_payload(payload)
        .await
        .map_err(error::ErrorBadRequest)?;

    let note = notebook_service::get_notebook_note(&user_db, &note_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook note not found"))?;

    let image_id = Uuid::new_v4().to_string();
    let public_id = format!(
        "tradstry/notebook/{}/{}/{}",
        user_db.user_id(),
        note.account_id,
        image_id
    );
    let data_url = format!(
        "data:{};base64,{}",
        mime_type,
        BASE64_STANDARD.encode(&bytes)
    );

    info!(
        "Uploading notebook image: user_id={} account_id={} note_id={} image_id={}",
        user_db.user_id(),
        note.account_id,
        note.id,
        image_id
    );

    let uploaded = cloudinary
        .upload_notebook_image(
            data_url,
            public_id,
            filename.clone(),
            vec![
                "tradstry".to_string(),
                "notebook".to_string(),
                format!("note:{}", note.id),
                format!("account:{}", note.account_id),
            ],
        )
        .await
        .map_err(error::ErrorInternalServerError)?;

    let image = image_service::create_notebook_image(
        &user_db,
        CreateNotebookImageInput {
            id: image_id,
            note_id: note.id,
            account_id: note.account_id,
            cloudinary_asset_id: uploaded.asset_id,
            cloudinary_public_id: uploaded.public_id,
            secure_url: uploaded.secure_url,
            width: uploaded.width,
            height: uploaded.height,
            format: uploaded.format,
            bytes: uploaded.bytes,
            original_filename: if uploaded.original_filename.is_empty() {
                filename
            } else {
                uploaded.original_filename
            },
        },
    )
    .await
    .map_err(error::ErrorInternalServerError)?;

    Ok(HttpResponse::Ok().json(UploadNotebookImageResponse { image }))
}

pub async fn get_notebook_image(
    req: HttpRequest,
    path: web::Path<String>,
    turso: web::Data<Arc<TursoClient>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, turso.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let image_id = path.into_inner();

    let image = image_service::get_notebook_image(&user_db, &image_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook image not found"))?;

    Ok(HttpResponse::Ok().json(image))
}

pub async fn delete_notebook_image(
    req: HttpRequest,
    path: web::Path<String>,
    turso: web::Data<Arc<TursoClient>>,
    cloudinary: web::Data<Arc<CloudinaryClient>>,
) -> Result<HttpResponse> {
    let user_db = get_user_db(&req, turso.get_ref())
        .await
        .map_err(error::ErrorUnauthorized)?;
    let image_id = path.into_inner();

    let image = image_service::get_notebook_image(&user_db, &image_id)
        .await
        .map_err(error::ErrorInternalServerError)?
        .ok_or_else(|| error::ErrorNotFound("Notebook image not found"))?;

    cloudinary
        .delete_notebook_image(image.cloudinary_public_id.clone())
        .await
        .map_err(error::ErrorInternalServerError)?;

    image_service::delete_notebook_image(&user_db, &image.id)
        .await
        .map_err(error::ErrorInternalServerError)?;

    Ok(HttpResponse::NoContent().finish())
}
