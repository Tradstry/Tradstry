use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use super::notes;

// NOTE: `cloudinary_public_id` now holds the R2 object key (the column name is
// kept to avoid a schema rebuild). `secure_url` is no longer the serving URL —
// the read path overwrites it with a freshly presigned R2 GET URL before
// returning records to clients.
const SELECT_COLS: &str = "id, note_id, user_id, workspace_id, cloudinary_asset_id, cloudinary_public_id, secure_url, width, height, format, bytes, original_filename, media_type, content_type, duration_seconds, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, content_hash";

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookImage {
    pub id: String,
    pub note_id: String,
    pub user_id: String,
    pub workspace_id: String,
    pub cloudinary_asset_id: String,
    pub cloudinary_public_id: String,
    pub secure_url: String,
    pub width: i64,
    pub height: i64,
    pub format: String,
    pub bytes: i64,
    pub original_filename: String,
    pub media_type: String,
    pub content_type: String,
    pub duration_seconds: f64,
    pub created_at: String,
    pub content_hash: String,
}

#[derive(Debug, Clone)]
pub struct CreateNotebookImageInput {
    pub id: String,
    pub note_id: String,
    pub workspace_id: String,
    pub cloudinary_asset_id: String,
    pub cloudinary_public_id: String,
    pub secure_url: String,
    pub width: i64,
    pub height: i64,
    pub format: String,
    pub bytes: i64,
    pub original_filename: String,
    pub media_type: String,
    pub content_type: String,
    pub duration_seconds: f64,
    pub content_hash: String,
}

fn row_to_notebook_image(row: &sqlx::postgres::PgRow) -> Result<NotebookImage> {
    Ok(NotebookImage {
        id: row.try_get::<String, _>(0)?,
        note_id: row.try_get::<String, _>(1)?,
        user_id: row.try_get::<String, _>(2)?,
        workspace_id: row.try_get::<String, _>(3)?,
        cloudinary_asset_id: row.try_get::<String, _>(4)?,
        cloudinary_public_id: row.try_get::<String, _>(5)?,
        secure_url: row.try_get::<String, _>(6)?,
        width: row.try_get::<i64, _>(7)?,
        height: row.try_get::<i64, _>(8)?,
        format: row.try_get::<String, _>(9)?,
        bytes: row.try_get::<i64, _>(10)?,
        original_filename: row.try_get::<String, _>(11)?,
        media_type: row.try_get::<String, _>(12)?,
        content_type: row.try_get::<String, _>(13)?,
        duration_seconds: row.try_get::<f64, _>(14)?,
        created_at: row.try_get::<String, _>(15)?,
        content_hash: row.try_get::<String, _>(16)?,
    })
}

pub async fn list_notebook_images_for_note(
    pool: &PgPool,
    note_id: &str,
    user_id: &str,
) -> Result<Vec<NotebookImage>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM notebook_images WHERE note_id = $1 AND user_id = $2 ORDER BY created_at ASC, id ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(note_id)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list notebook images")?;

    let mut images = Vec::new();
    for row in &rows {
        images.push(row_to_notebook_image(row)?);
    }

    Ok(images)
}

/// Batched sibling of `list_notebook_images_for_note`: fetches every image for a
/// set of notes in a single query, returning `(note_id, image)` pairs so callers
/// can group them without an extra round-trip per note.
///
/// `note_id` is appended as a trailing select column (index 17). Prepending it
/// would shift every column and break `row_to_notebook_image`, which reads the
/// existing `SELECT_COLS` layout by hardcoded index 0..=16; appending leaves that
/// mapper untouched. The `ORDER BY note_id, created_at ASC, id ASC` preserves the
/// exact per-note ordering of the single-note function within each note's group.
pub async fn list_notebook_images_for_notes(
    pool: &PgPool,
    note_ids: &[String],
    user_id: &str,
) -> Result<Vec<(String, NotebookImage)>> {
    let sql = format!(
        "SELECT {SELECT_COLS}, note_id FROM notebook_images WHERE note_id = ANY($1) AND user_id = $2 ORDER BY note_id, created_at ASC, id ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(note_ids)
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list notebook images for notes")?;

    let mut images = Vec::new();
    for row in &rows {
        let image = row_to_notebook_image(row)?;
        let note_id = row.try_get::<String, _>(17)?;
        images.push((note_id, image));
    }

    Ok(images)
}

pub async fn find_notebook_image(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<NotebookImage>> {
    let sql = format!("SELECT {SELECT_COLS} FROM notebook_images WHERE id = $1 AND user_id = $2");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find notebook image")?;

    match row {
        Some(row) => Ok(Some(row_to_notebook_image(&row)?)),
        None => Ok(None),
    }
}

/// Any image the user owns whose bytes hash to `content_hash`, regardless of note.
/// The bytes are content-addressed and deduped in R2, so any matching row resolves
/// the same object; the read path presigns a fresh URL from `cloudinary_public_id`.
pub async fn find_notebook_image_by_hash(
    pool: &PgPool,
    user_id: &str,
    content_hash: &str,
) -> Result<Option<NotebookImage>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM notebook_images WHERE user_id = $1 AND content_hash = $2 ORDER BY created_at ASC, id ASC LIMIT 1"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(content_hash)
        .fetch_optional(pool)
        .await
        .context("Failed to find notebook image by hash")?;

    match row {
        Some(row) => Ok(Some(row_to_notebook_image(&row)?)),
        None => Ok(None),
    }
}

/// The row for one specific `(note, hash)` pair. Upload is idempotent on this:
/// re-pasting the same bytes into the same note returns the existing row.
pub async fn find_notebook_image_for_note_hash(
    pool: &PgPool,
    user_id: &str,
    note_id: &str,
    content_hash: &str,
) -> Result<Option<NotebookImage>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM notebook_images WHERE user_id = $1 AND note_id = $2 AND content_hash = $3 LIMIT 1"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(note_id)
        .bind(content_hash)
        .fetch_optional(pool)
        .await
        .context("Failed to find notebook image for note+hash")?;

    match row {
        Some(row) => Ok(Some(row_to_notebook_image(&row)?)),
        None => Ok(None),
    }
}

/// How many rows the user still has pointing at these bytes. The R2 object is only
/// safe to delete when this reaches zero.
pub async fn count_images_with_hash(
    pool: &PgPool,
    user_id: &str,
    content_hash: &str,
) -> Result<i64> {
    let n: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notebook_images WHERE user_id = $1 AND content_hash = $2",
    )
    .bind(user_id)
    .bind(content_hash)
    .fetch_one(pool)
    .await
    .context("Failed to count images with hash")?;
    Ok(n)
}

pub async fn delete_notebook_image(pool: &PgPool, id: &str, user_id: &str) -> Result<()> {
    sqlx::query("DELETE FROM notebook_images WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete notebook image")?;

    Ok(())
}

pub async fn create_notebook_image(
    pool: &PgPool,
    user_id: &str,
    input: CreateNotebookImageInput,
) -> Result<NotebookImage> {
    let note = notes::find_notebook_note(pool, &input.note_id, user_id)
        .await?
        .ok_or_else(|| anyhow!("Notebook note '{}' not found", input.note_id))?;

    ensure!(
        note.workspace_id == input.workspace_id,
        "Notebook note '{}' does not belong to account '{}'",
        input.note_id,
        input.workspace_id
    );

    sqlx::query(
        r#"
        INSERT INTO notebook_images (
            id,
            note_id,
            user_id,
            workspace_id,
            cloudinary_asset_id,
            cloudinary_public_id,
            secure_url,
            width,
            height,
            format,
            bytes,
            original_filename,
            media_type,
            content_type,
            duration_seconds,
            content_hash
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        "#,
    )
    .bind(input.id.as_str())
    .bind(input.note_id.as_str())
    .bind(user_id)
    .bind(input.workspace_id.as_str())
    .bind(input.cloudinary_asset_id.as_str())
    .bind(input.cloudinary_public_id.as_str())
    .bind(input.secure_url.as_str())
    .bind(input.width)
    .bind(input.height)
    .bind(input.format.as_str())
    .bind(input.bytes)
    .bind(input.original_filename.as_str())
    .bind(input.media_type.as_str())
    .bind(input.content_type.as_str())
    .bind(input.duration_seconds)
    .bind(input.content_hash.as_str())
    .execute(pool)
    .await
    .context("Failed to insert notebook image")?;

    find_notebook_image(pool, &input.id, user_id)
        .await?
        .context("Notebook image not found after insert")
}

pub async fn sync_note_image_workspace_id(
    pool: &PgPool,
    note_id: &str,
    user_id: &str,
    workspace_id: &str,
) -> Result<()> {
    sqlx::query("UPDATE notebook_images SET workspace_id = $1 WHERE note_id = $2 AND user_id = $3")
        .bind(workspace_id)
        .bind(note_id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to sync notebook image account ids")?;

    Ok(())
}
