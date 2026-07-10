use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
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
    pub id: Option<String>,
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
        "SELECT {SELECT_COLS} FROM notebook_folders WHERE account_id = $1 AND deleted_at IS NULL ORDER BY sort_order ASC, name ASC"
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
    let sql =
        format!("SELECT {SELECT_COLS} FROM notebook_folders WHERE id = $1 AND deleted_at IS NULL");
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

async fn next_folder_sort_order(
    conn: &mut PgConnection,
    account_id: &str,
    parent_folder_id: Option<&str>,
) -> Result<i64> {
    // NULL-binding choice: branch on `is_none()` to use `IS NULL` vs `= $2`,
    // rather than binding `Option::None` against an `IS $` placeholder.
    let row = if let Some(parent_id) = parent_folder_id {
        sqlx::query(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook_folders WHERE account_id = $1 AND parent_folder_id = $2 AND deleted_at IS NULL",
        )
        .bind(account_id)
        .bind(parent_id)
        .fetch_optional(&mut *conn)
        .await
    } else {
        sqlx::query(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook_folders WHERE account_id = $1 AND parent_folder_id IS NULL AND deleted_at IS NULL",
        )
        .bind(account_id)
        .fetch_optional(&mut *conn)
        .await
    }
    .context("Failed to compute next folder sort_order")?;

    match row {
        Some(row) => Ok(row.try_get::<i64, _>(0)?),
        None => Ok(0),
    }
}

/// Transactional core: inserts the folder on the caller's connection with an
/// explicit `sort_order` and HLC stamp ("" for server-authored rows).
pub async fn create_notebook_folder_tx(
    conn: &mut PgConnection,
    input: CreateNotebookFolderInput,
    sort_order: i64,
    hlc: &str,
) -> Result<String> {
    let id = match input.id.as_deref() {
        Some(id) => {
            Uuid::parse_str(id).context("Client-supplied folder id must be a UUID")?;
            id.to_string()
        }
        None => Uuid::new_v4().to_string(),
    };

    sqlx::query(
        r#"
        INSERT INTO notebook_folders (id, user_id, account_id, parent_folder_id, name, sort_order, hlc)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(id.as_str())
    .bind(input.user_id.as_str())
    .bind(input.account_id.as_str())
    .bind(input.parent_folder_id.as_deref())
    .bind(input.name.as_str())
    .bind(sort_order)
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("Failed to insert notebook folder")?;

    Ok(id)
}

pub async fn create_notebook_folder(
    pool: &PgPool,
    input: CreateNotebookFolderInput,
) -> Result<NotebookFolder> {
    let mut tx = pool.begin().await?;
    let sort_order = next_folder_sort_order(
        &mut tx,
        &input.account_id,
        input.parent_folder_id.as_deref(),
    )
    .await?;
    let id = create_notebook_folder_tx(&mut tx, input, sort_order, "").await?;
    tx.commit().await?;

    find_notebook_folder(pool, &id)
        .await?
        .context("Notebook folder not found after insert")
}

pub async fn rename_notebook_folder_tx(
    conn: &mut PgConnection,
    id: &str,
    name: &str,
    hlc: &str,
) -> Result<()> {
    sqlx::query("UPDATE notebook_folders SET name = $2, hlc = $3 WHERE id = $1")
        .bind(id)
        .bind(name)
        .bind(hlc)
        .execute(&mut *conn)
        .await
        .context("Failed to rename notebook folder")?;

    Ok(())
}

pub async fn rename_notebook_folder(pool: &PgPool, id: &str, name: &str) -> Result<()> {
    let mut conn = pool.acquire().await?;
    rename_notebook_folder_tx(&mut conn, id, name, "").await
}

pub async fn folder_subtree_ids<'e, E>(executor: E, folder_id: &str) -> Result<Vec<String>>
where
    E: sqlx::PgExecutor<'e>,
{
    // No `deleted_at` guard: deleting a folder whose child was already tombstoned
    // must still stamp that child, or the subtree delete would leave orphans.
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
    .fetch_all(executor)
    .await
    .context("Failed to gather folder subtree ids")?;

    let mut ids = Vec::new();
    for row in &rows {
        ids.push(row.try_get::<String, _>(0)?);
    }

    Ok(ids)
}

pub async fn move_notebook_node_tx(
    conn: &mut PgConnection,
    input: MoveNotebookNodeInput,
    hlc: &str,
) -> Result<()> {
    // Cycle guard: a folder cannot be moved into itself or any of its descendants.
    if input.node_type == NotebookNodeType::Folder
        && let Some(target) = input.new_parent_folder_id.as_deref()
    {
        let subtree = folder_subtree_ids(&mut *conn, &input.node_id).await?;
        if subtree.iter().any(|id| id == target) {
            anyhow::bail!("cannot move a folder into itself or a descendant");
        }
    }

    let sql = match input.node_type {
        NotebookNodeType::Folder => {
            "UPDATE notebook_folders SET parent_folder_id = $2, sort_order = $3, hlc = $4 WHERE id = $1"
        }
        NotebookNodeType::Note => {
            "UPDATE notebook_notes SET folder_id = $2, sort_order = $3, hlc = $4 WHERE id = $1"
        }
    };

    sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(input.node_id.as_str())
        .bind(input.new_parent_folder_id.as_deref())
        .bind(input.new_sort_order)
        .bind(hlc)
        .execute(&mut *conn)
        .await
        .context("Failed to reparent notebook node")?;

    renumber_sibling_group(conn, &input.account_id, &input.new_parent_folder_id).await?;

    Ok(())
}

pub async fn move_notebook_node(pool: &PgPool, input: MoveNotebookNodeInput) -> Result<()> {
    // Reparent the node and renumber the destination sibling group together so
    // the two writes either both land or both roll back.
    let mut tx = pool.begin().await?;
    move_notebook_node_tx(&mut tx, input, "").await?;
    tx.commit().await?;

    Ok(())
}

/// Renumber a combined sibling group (folders + notes sharing the same parent)
/// to contiguous `sort_order` values 0..N.
///
/// NULL-binding choice: branch on `is_none()` to use `IS NULL` vs `= $2`.
async fn renumber_sibling_group(
    conn: &mut PgConnection,
    account_id: &str,
    parent_folder_id: &Option<String>,
) -> Result<()> {
    let rows = if let Some(parent_id) = parent_folder_id.as_deref() {
        sqlx::query(
            r#"
            SELECT 'folder' AS kind, id, sort_order, created_at FROM notebook_folders
                WHERE account_id = $1 AND parent_folder_id = $2 AND deleted_at IS NULL
            UNION ALL
            SELECT 'note' AS kind, id, sort_order, created_at FROM notebook_notes
                WHERE account_id = $1 AND folder_id = $2 AND deleted_at IS NULL
            ORDER BY sort_order ASC, created_at ASC
            "#,
        )
        .bind(account_id)
        .bind(parent_id)
        .fetch_all(&mut *conn)
        .await
    } else {
        sqlx::query(
            r#"
            SELECT 'folder' AS kind, id, sort_order, created_at FROM notebook_folders
                WHERE account_id = $1 AND parent_folder_id IS NULL AND deleted_at IS NULL
            UNION ALL
            SELECT 'note' AS kind, id, sort_order, created_at FROM notebook_notes
                WHERE account_id = $1 AND folder_id IS NULL AND deleted_at IS NULL
            ORDER BY sort_order ASC, created_at ASC
            "#,
        )
        .bind(account_id)
        .fetch_all(&mut *conn)
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
            .execute(&mut *conn)
            .await
            .context("Failed to renumber sibling sort_order")?;
    }

    Ok(())
}

pub async fn gather_subtree_image_public_ids(
    pool: &PgPool,
    folder_id: &str,
) -> Result<Vec<String>> {
    // No `deleted_at` guard on the folders/notes here: already-tombstoned notes
    // still own R2 images, and this is the only path that reaps them.
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

pub async fn delete_notebook_folder_subtree_tx(
    conn: &mut PgConnection,
    folder_id: &str,
    hlc: &str,
) -> Result<()> {
    // Soft delete does not fire ON DELETE CASCADE, so stamp both the whole folder
    // subtree and every note inside it explicitly.
    let ids = folder_subtree_ids(&mut *conn, folder_id).await?;

    sqlx::query(
        "UPDATE notebook_folders SET deleted_at = now(), hlc = $2 WHERE id = ANY($1) AND deleted_at IS NULL",
    )
    .bind(&ids)
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("Failed to soft-delete folder subtree")?;

    sqlx::query("UPDATE notebook_notes SET deleted_at = now(), hlc = $2 WHERE folder_id = ANY($1) AND deleted_at IS NULL")
        .bind(&ids)
        .bind(hlc)
        .execute(&mut *conn)
        .await
        .context("Failed to soft-delete notes in folder subtree")?;

    Ok(())
}

pub async fn delete_notebook_folder_subtree(pool: &PgPool, folder_id: &str) -> Result<()> {
    let mut tx = pool.begin().await?;
    delete_notebook_folder_subtree_tx(&mut tx, folder_id, "").await?;
    tx.commit().await?;
    Ok(())
}
