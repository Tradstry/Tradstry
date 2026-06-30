use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};

use super::notebook_table;

// NOTE: `cloudinary_public_id` now holds the R2 object key (the column name is
// kept to avoid a schema rebuild). `secure_url` is no longer the serving URL —
// the read path overwrites it with a freshly presigned R2 GET URL before
// returning records to clients.
const SELECT_COLS: &str = "id, note_id, user_id, account_id, cloudinary_asset_id, cloudinary_public_id, secure_url, width, height, format, bytes, original_filename, media_type, content_type, duration_seconds, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at";

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookImage {
    pub id: String,
    pub note_id: String,
    pub user_id: String,
    pub account_id: String,
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
}

#[derive(Debug, Clone)]
pub struct CreateNotebookImageInput {
    pub id: String,
    pub note_id: String,
    pub account_id: String,
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
}

fn row_to_notebook_image(row: &sqlx::postgres::PgRow) -> Result<NotebookImage> {
    Ok(NotebookImage {
        id: row.try_get::<String, _>(0)?,
        note_id: row.try_get::<String, _>(1)?,
        user_id: row.try_get::<String, _>(2)?,
        account_id: row.try_get::<String, _>(3)?,
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
    let note = notebook_table::find_notebook_note(pool, &input.note_id, user_id)
        .await?
        .ok_or_else(|| anyhow!("Notebook note '{}' not found", input.note_id))?;

    ensure!(
        note.account_id == input.account_id,
        "Notebook note '{}' does not belong to account '{}'",
        input.note_id,
        input.account_id
    );

    sqlx::query(
        r#"
        INSERT INTO notebook_images (
            id,
            note_id,
            user_id,
            account_id,
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
            duration_seconds
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
        "#,
    )
    .bind(input.id.as_str())
    .bind(input.note_id.as_str())
    .bind(user_id)
    .bind(input.account_id.as_str())
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
    .execute(pool)
    .await
    .context("Failed to insert notebook image")?;

    find_notebook_image(pool, &input.id, user_id)
        .await?
        .context("Notebook image not found after insert")
}

pub async fn sync_note_image_account_id(
    pool: &PgPool,
    note_id: &str,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    sqlx::query("UPDATE notebook_images SET account_id = $1 WHERE note_id = $2 AND user_id = $3")
        .bind(account_id)
        .bind(note_id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to sync notebook image account ids")?;

    Ok(())
}
