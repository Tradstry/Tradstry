use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::SimpleObject;
use libsql::Connection;
use serde::{Deserialize, Serialize};

use super::notebook_table;

// NOTE: `cloudinary_public_id` now holds the R2 object key (the column name is
// kept to avoid a schema rebuild). `secure_url` is no longer the serving URL —
// the read path overwrites it with a freshly presigned R2 GET URL before
// returning records to clients.
const SELECT_COLS: &str = "id, note_id, user_id, account_id, cloudinary_asset_id, cloudinary_public_id, secure_url, width, height, format, bytes, original_filename, media_type, content_type, duration_seconds, created_at";

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

fn row_to_notebook_image(row: &libsql::Row) -> Result<NotebookImage> {
    Ok(NotebookImage {
        id: row.get::<String>(0)?,
        note_id: row.get::<String>(1)?,
        user_id: row.get::<String>(2)?,
        account_id: row.get::<String>(3)?,
        cloudinary_asset_id: row.get::<String>(4)?,
        cloudinary_public_id: row.get::<String>(5)?,
        secure_url: row.get::<String>(6)?,
        width: row.get::<i64>(7)?,
        height: row.get::<i64>(8)?,
        format: row.get::<String>(9)?,
        bytes: row.get::<i64>(10)?,
        original_filename: row.get::<String>(11)?,
        media_type: row.get::<String>(12)?,
        content_type: row.get::<String>(13)?,
        duration_seconds: row.get::<f64>(14)?,
        created_at: row.get::<String>(15)?,
    })
}

pub async fn list_notebook_images_for_note(
    conn: &Connection,
    note_id: &str,
    user_id: &str,
) -> Result<Vec<NotebookImage>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM notebook_images WHERE note_id = ?1 AND user_id = ?2 ORDER BY created_at ASC, id ASC"
            ),
            libsql::params![note_id, user_id],
        )
        .await
        .context("Failed to list notebook images")?;

    let mut images = Vec::new();
    while let Some(row) = rows.next().await? {
        images.push(row_to_notebook_image(&row)?);
    }

    Ok(images)
}

pub async fn find_notebook_image(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<NotebookImage>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM notebook_images WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find notebook image")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_notebook_image(&row)?)),
        None => Ok(None),
    }
}

pub async fn delete_notebook_image(conn: &Connection, id: &str, user_id: &str) -> Result<()> {
    conn.execute(
        "DELETE FROM notebook_images WHERE id = ?1 AND user_id = ?2",
        libsql::params![id, user_id],
    )
    .await
    .context("Failed to delete notebook image")?;

    Ok(())
}

pub async fn create_notebook_image(
    conn: &Connection,
    user_id: &str,
    input: CreateNotebookImageInput,
) -> Result<NotebookImage> {
    let note = notebook_table::find_notebook_note(conn, &input.note_id, user_id)
        .await?
        .ok_or_else(|| anyhow!("Notebook note '{}' not found", input.note_id))?;

    ensure!(
        note.account_id == input.account_id,
        "Notebook note '{}' does not belong to account '{}'",
        input.note_id,
        input.account_id
    );

    conn.execute(
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
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
        "#,
        libsql::params![
            input.id.as_str(),
            input.note_id.as_str(),
            user_id,
            input.account_id.as_str(),
            input.cloudinary_asset_id.as_str(),
            input.cloudinary_public_id.as_str(),
            input.secure_url.as_str(),
            input.width,
            input.height,
            input.format.as_str(),
            input.bytes,
            input.original_filename.as_str(),
            input.media_type.as_str(),
            input.content_type.as_str(),
            input.duration_seconds,
        ],
    )
    .await
    .context("Failed to insert notebook image")?;

    find_notebook_image(conn, &input.id, user_id)
        .await?
        .context("Notebook image not found after insert")
}

pub async fn sync_note_image_account_id(
    conn: &Connection,
    note_id: &str,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    conn.execute(
        "UPDATE notebook_images SET account_id = ?1 WHERE note_id = ?2 AND user_id = ?3",
        libsql::params![account_id, note_id, user_id],
    )
    .await
    .context("Failed to sync notebook image account ids")?;

    Ok(())
}
