use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const SELECT_COLS: &str = "id, user_id, account_id, parent_folder_id, name, sort_order, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[serde(rename_all = "camelCase")]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookFolder {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

pub struct CreateNotebookFolderInput {
    pub user_id: String,
    pub account_id: String,
    pub parent_folder_id: Option<String>,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotebookNodeType {
    Folder,
    Note,
}

pub struct MoveNotebookNodeInput {
    pub account_id: String,
    pub node_id: String,
    pub node_type: NotebookNodeType,
    pub new_parent_folder_id: Option<String>,
    pub new_sort_order: i64,
}

fn opt_text(row: &sqlx::postgres::PgRow, idx: usize) -> Option<String> {
    row.try_get::<Option<String>, _>(idx)
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

fn row_to_notebook_folder(row: &sqlx::postgres::PgRow) -> Result<NotebookFolder> {
    Ok(NotebookFolder {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        parent_folder_id: opt_text(row, 3),
        name: row.try_get::<String, _>(4)?,
        sort_order: row.try_get::<i64, _>(5)?,
        created_at: row.try_get::<String, _>(6)?,
        updated_at: row.try_get::<String, _>(7)?,
    })
}

pub async fn list_notebook_folders(pool: &PgPool, account_id: &str) -> Result<Vec<NotebookFolder>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM notebook_folders WHERE account_id = $1 ORDER BY sort_order ASC, name ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to list notebook folders")?;

    let mut folders = Vec::new();
    for row in &rows {
        folders.push(row_to_notebook_folder(row)?);
    }

    Ok(folders)
}

pub async fn find_notebook_folder(pool: &PgPool, id: &str) -> Result<Option<NotebookFolder>> {
    let sql = format!("SELECT {SELECT_COLS} FROM notebook_folders WHERE id = $1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await
        .context("Failed to find notebook folder")?;

    match row {
        Some(row) => Ok(Some(row_to_notebook_folder(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_notebook_folder(
    pool: &PgPool,
    input: CreateNotebookFolderInput,
) -> Result<NotebookFolder> {
    let id = Uuid::new_v4().to_string();

    // NULL-binding choice: branch on `is_none()` to use `IS NULL` vs `= $2`,
    // rather than binding `Option::None` against an `IS $` placeholder.
    let next_sort_order: i64 = {
        let row = if let Some(parent_id) = input.parent_folder_id.as_deref() {
            sqlx::query(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook_folders WHERE account_id = $1 AND parent_folder_id = $2",
            )
            .bind(input.account_id.as_str())
            .bind(parent_id)
            .fetch_optional(pool)
            .await
        } else {
            sqlx::query(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook_folders WHERE account_id = $1 AND parent_folder_id IS NULL",
            )
            .bind(input.account_id.as_str())
            .fetch_optional(pool)
            .await
        }
        .context("Failed to compute next folder sort_order")?;

        match row {
            Some(row) => row.try_get::<i64, _>(0)?,
            None => 0,
        }
    };

    sqlx::query(
        r#"
        INSERT INTO notebook_folders (id, user_id, account_id, parent_folder_id, name, sort_order)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id.as_str())
    .bind(input.user_id.as_str())
    .bind(input.account_id.as_str())
    .bind(input.parent_folder_id.as_deref())
    .bind(input.name.as_str())
    .bind(next_sort_order)
    .execute(pool)
    .await
    .context("Failed to insert notebook folder")?;

    find_notebook_folder(pool, &id)
        .await?
        .context("Notebook folder not found after insert")
}

pub async fn rename_notebook_folder(pool: &PgPool, id: &str, name: &str) -> Result<()> {
    sqlx::query("UPDATE notebook_folders SET name = $2 WHERE id = $1")
        .bind(id)
        .bind(name)
        .execute(pool)
        .await
        .context("Failed to rename notebook folder")?;

    Ok(())
}

pub async fn folder_subtree_ids(pool: &PgPool, folder_id: &str) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
            WITH RECURSIVE subtree(id) AS (
                SELECT id FROM notebook_folders WHERE id = $1
                UNION ALL
                SELECT f.id FROM notebook_folders f JOIN subtree s ON f.parent_folder_id = s.id
            ) SELECT id FROM subtree
            "#,
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .context("Failed to gather folder subtree ids")?;

    let mut ids = Vec::new();
    for row in &rows {
        ids.push(row.try_get::<String, _>(0)?);
    }

    Ok(ids)
}

pub async fn move_notebook_node(pool: &PgPool, input: MoveNotebookNodeInput) -> Result<()> {
    // Cycle guard: a folder cannot be moved into itself or any of its descendants.
    if input.node_type == NotebookNodeType::Folder
        && let Some(target) = input.new_parent_folder_id.as_deref()
    {
        let subtree = folder_subtree_ids(pool, &input.node_id).await?;
        if subtree.iter().any(|id| id == target) {
            anyhow::bail!("cannot move a folder into itself or a descendant");
        }
    }

    let sql = match input.node_type {
        NotebookNodeType::Folder => {
            "UPDATE notebook_folders SET parent_folder_id = $2, sort_order = $3 WHERE id = $1"
        }
        NotebookNodeType::Note => {
            "UPDATE notebook_notes SET folder_id = $2, sort_order = $3 WHERE id = $1"
        }
    };

    // Reparent the node and renumber the destination sibling group together so
    // the two writes either both land or both roll back.
    let mut tx = pool.begin().await?;

    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(input.node_id.as_str())
        .bind(input.new_parent_folder_id.as_deref())
        .bind(input.new_sort_order)
        .execute(&mut *tx)
        .await
        .context("Failed to reparent notebook node")?;

    renumber_sibling_group(&mut tx, &input.account_id, &input.new_parent_folder_id).await?;

    tx.commit().await?;

    Ok(())
}

/// Renumber a combined sibling group (folders + notes sharing the same parent)
/// to contiguous `sort_order` values 0..N.
///
/// NULL-binding choice: branch on `is_none()` to use `IS NULL` vs `= $2`.
async fn renumber_sibling_group(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    account_id: &str,
    parent_folder_id: &Option<String>,
) -> Result<()> {
    let rows = if let Some(parent_id) = parent_folder_id.as_deref() {
        sqlx::query(
            r#"
            SELECT 'folder' AS kind, id, sort_order, created_at FROM notebook_folders
                WHERE account_id = $1 AND parent_folder_id = $2
            UNION ALL
            SELECT 'note' AS kind, id, sort_order, created_at FROM notebook_notes
                WHERE account_id = $1 AND folder_id = $2
            ORDER BY sort_order ASC, created_at ASC
            "#,
        )
        .bind(account_id)
        .bind(parent_id)
        .fetch_all(&mut **tx)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT 'folder' AS kind, id, sort_order, created_at FROM notebook_folders
                WHERE account_id = $1 AND parent_folder_id IS NULL
            UNION ALL
            SELECT 'note' AS kind, id, sort_order, created_at FROM notebook_notes
                WHERE account_id = $1 AND folder_id IS NULL
            ORDER BY sort_order ASC, created_at ASC
            "#,
        )
        .bind(account_id)
        .fetch_all(&mut **tx)
        .await
    }
    .context("Failed to load sibling group for renumbering")?;

    let mut siblings: Vec<(String, String)> = Vec::new();
    for row in &rows {
        let kind = row.try_get::<String, _>(0)?;
        let id = row.try_get::<String, _>(1)?;
        siblings.push((kind, id));
    }

    for (index, (kind, id)) in siblings.into_iter().enumerate() {
        let new_order = index as i64;
        let update_sql = match kind.as_str() {
            "folder" => "UPDATE notebook_folders SET sort_order = $2 WHERE id = $1",
            _ => "UPDATE notebook_notes SET sort_order = $2 WHERE id = $1",
        };

        sqlx::query(sqlx::AssertSqlSafe(update_sql))
            .bind(id.as_str())
            .bind(new_order)
            .execute(&mut **tx)
            .await
            .context("Failed to renumber sibling sort_order")?;
    }

    Ok(())
}

pub async fn gather_subtree_image_public_ids(
    pool: &PgPool,
    folder_id: &str,
) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
            WITH RECURSIVE subtree(id) AS (
                SELECT id FROM notebook_folders WHERE id = $1
                UNION ALL
                SELECT f.id FROM notebook_folders f JOIN subtree s ON f.parent_folder_id = s.id
            )
            SELECT i.cloudinary_public_id FROM notebook_images i
            JOIN notebook_notes n ON i.note_id = n.id
            WHERE n.folder_id IN (SELECT id FROM subtree)
            "#,
    )
    .bind(folder_id)
    .fetch_all(pool)
    .await
    .context("Failed to gather subtree image public ids")?;

    let mut public_ids = Vec::new();
    for row in &rows {
        public_ids.push(row.try_get::<String, _>(0)?);
    }

    Ok(public_ids)
}

pub async fn delete_notebook_folder_subtree(pool: &PgPool, folder_id: &str) -> Result<()> {
    // The FK on notebook_notes.folder_id is NOT enforced on pre-v0.4 DBs, so
    // delete the notes explicitly BEFORE deleting folders. This cascades to
    // notebook_images + notebook_note_trades via their enforced note_id FKs.
    // Both deletes run in one transaction so they commit or roll back together.
    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        DELETE FROM notebook_notes WHERE folder_id IN (
            WITH RECURSIVE subtree(id) AS (
                SELECT id FROM notebook_folders WHERE id = $1
                UNION ALL
                SELECT f.id FROM notebook_folders f JOIN subtree s ON f.parent_folder_id = s.id
            )
            SELECT id FROM subtree
        )
        "#,
    )
    .bind(folder_id)
    .execute(&mut *tx)
    .await
    .context("Failed to delete notes within folder subtree")?;

    // The self-referential FK on notebook_folders (new table, enforced) cascades
    // the delete to all descendant folders.
    sqlx::query("DELETE FROM notebook_folders WHERE id = $1")
        .bind(folder_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete notebook folder subtree")?;

    tx.commit().await?;

    Ok(())
}
