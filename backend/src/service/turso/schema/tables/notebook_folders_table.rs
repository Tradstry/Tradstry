use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use libsql::Connection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const SELECT_COLS: &str =
    "id, user_id, account_id, parent_folder_id, name, sort_order, created_at, updated_at";

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

fn opt_text(row: &libsql::Row, idx: i32) -> Option<String> {
    row.get::<libsql::Value>(idx).ok().and_then(|v| match v {
        libsql::Value::Text(s) if !s.is_empty() => Some(s),
        _ => None,
    })
}

fn row_to_notebook_folder(row: &libsql::Row) -> Result<NotebookFolder> {
    Ok(NotebookFolder {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        parent_folder_id: opt_text(row, 3),
        name: row.get::<String>(4)?,
        sort_order: row.get::<i64>(5)?,
        created_at: row.get::<String>(6)?,
        updated_at: row.get::<String>(7)?,
    })
}

pub async fn list_notebook_folders(
    conn: &Connection,
    account_id: &str,
) -> Result<Vec<NotebookFolder>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM notebook_folders WHERE account_id = ?1 ORDER BY sort_order ASC, name ASC"
            ),
            libsql::params![account_id],
        )
        .await
        .context("Failed to list notebook folders")?;

    let mut folders = Vec::new();
    while let Some(row) = rows.next().await? {
        folders.push(row_to_notebook_folder(&row)?);
    }

    Ok(folders)
}

pub async fn find_notebook_folder(conn: &Connection, id: &str) -> Result<Option<NotebookFolder>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM notebook_folders WHERE id = ?1"),
            libsql::params![id],
        )
        .await
        .context("Failed to find notebook folder")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_notebook_folder(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_notebook_folder(
    conn: &Connection,
    input: CreateNotebookFolderInput,
) -> Result<NotebookFolder> {
    let id = Uuid::new_v4().to_string();

    // NULL-binding choice: branch on `is_none()` to use `IS NULL` vs `= ?2`,
    // rather than binding `Option::None` against an `IS ?` placeholder.
    let next_sort_order: i64 = {
        let mut rows = if let Some(parent_id) = input.parent_folder_id.as_deref() {
            conn.query(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook_folders WHERE account_id = ?1 AND parent_folder_id = ?2",
                libsql::params![input.account_id.as_str(), parent_id],
            )
            .await
        } else {
            conn.query(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM notebook_folders WHERE account_id = ?1 AND parent_folder_id IS NULL",
                libsql::params![input.account_id.as_str()],
            )
            .await
        }
        .context("Failed to compute next folder sort_order")?;

        match rows.next().await? {
            Some(row) => row.get::<i64>(0)?,
            None => 0,
        }
    };

    let parent_value = match input.parent_folder_id.as_deref() {
        Some(parent_id) => libsql::Value::Text(parent_id.to_string()),
        None => libsql::Value::Null,
    };

    conn.execute(
        r#"
        INSERT INTO notebook_folders (id, user_id, account_id, parent_folder_id, name, sort_order)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        libsql::params![
            id.as_str(),
            input.user_id.as_str(),
            input.account_id.as_str(),
            parent_value,
            input.name.as_str(),
            next_sort_order,
        ],
    )
    .await
    .context("Failed to insert notebook folder")?;

    find_notebook_folder(conn, &id)
        .await?
        .context("Notebook folder not found after insert")
}

pub async fn rename_notebook_folder(conn: &Connection, id: &str, name: &str) -> Result<()> {
    conn.execute(
        "UPDATE notebook_folders SET name = ?2 WHERE id = ?1",
        libsql::params![id, name],
    )
    .await
    .context("Failed to rename notebook folder")?;

    Ok(())
}

pub async fn folder_subtree_ids(conn: &Connection, folder_id: &str) -> Result<Vec<String>> {
    let mut rows = conn
        .query(
            r#"
            WITH RECURSIVE subtree(id) AS (
                SELECT id FROM notebook_folders WHERE id = ?1
                UNION ALL
                SELECT f.id FROM notebook_folders f JOIN subtree s ON f.parent_folder_id = s.id
            ) SELECT id FROM subtree
            "#,
            libsql::params![folder_id],
        )
        .await
        .context("Failed to gather folder subtree ids")?;

    let mut ids = Vec::new();
    while let Some(row) = rows.next().await? {
        ids.push(row.get::<String>(0)?);
    }

    Ok(ids)
}

pub async fn move_notebook_node(conn: &Connection, input: MoveNotebookNodeInput) -> Result<()> {
    // Cycle guard: a folder cannot be moved into itself or any of its descendants.
    if input.node_type == NotebookNodeType::Folder
        && let Some(target) = input.new_parent_folder_id.as_deref()
    {
        let subtree = folder_subtree_ids(conn, &input.node_id).await?;
        if subtree.iter().any(|id| id == target) {
            anyhow::bail!("cannot move a folder into itself or a descendant");
        }
    }

    // Reparent the node. Branch on `is_none()` for the NULL bind.
    let parent_value = match input.new_parent_folder_id.as_deref() {
        Some(parent_id) => libsql::Value::Text(parent_id.to_string()),
        None => libsql::Value::Null,
    };

    let sql = match input.node_type {
        NotebookNodeType::Folder => {
            "UPDATE notebook_folders SET parent_folder_id = ?2, sort_order = ?3 WHERE id = ?1"
        }
        NotebookNodeType::Note => {
            "UPDATE notebook_notes SET folder_id = ?2, sort_order = ?3 WHERE id = ?1"
        }
    };

    conn.execute(
        sql,
        libsql::params![input.node_id.as_str(), parent_value, input.new_sort_order,],
    )
    .await
    .context("Failed to reparent notebook node")?;

    renumber_sibling_group(conn, &input.account_id, &input.new_parent_folder_id).await?;

    Ok(())
}

/// Renumber a combined sibling group (folders + notes sharing the same parent)
/// to contiguous `sort_order` values 0..N.
///
/// NULL-binding choice: branch on `is_none()` to use `IS NULL` vs `= ?2`.
async fn renumber_sibling_group(
    conn: &Connection,
    account_id: &str,
    parent_folder_id: &Option<String>,
) -> Result<()> {
    let mut rows = if let Some(parent_id) = parent_folder_id.as_deref() {
        conn.query(
            r#"
            SELECT 'folder' AS kind, id, sort_order, created_at FROM notebook_folders
                WHERE account_id = ?1 AND parent_folder_id = ?2
            UNION ALL
            SELECT 'note' AS kind, id, sort_order, created_at FROM notebook_notes
                WHERE account_id = ?1 AND folder_id = ?2
            ORDER BY sort_order ASC, created_at ASC
            "#,
            libsql::params![account_id, parent_id],
        )
        .await
    } else {
        conn.query(
            r#"
            SELECT 'folder' AS kind, id, sort_order, created_at FROM notebook_folders
                WHERE account_id = ?1 AND parent_folder_id IS NULL
            UNION ALL
            SELECT 'note' AS kind, id, sort_order, created_at FROM notebook_notes
                WHERE account_id = ?1 AND folder_id IS NULL
            ORDER BY sort_order ASC, created_at ASC
            "#,
            libsql::params![account_id],
        )
        .await
    }
    .context("Failed to load sibling group for renumbering")?;

    let mut siblings: Vec<(String, String)> = Vec::new();
    while let Some(row) = rows.next().await? {
        let kind = row.get::<String>(0)?;
        let id = row.get::<String>(1)?;
        siblings.push((kind, id));
    }

    for (index, (kind, id)) in siblings.into_iter().enumerate() {
        let new_order = index as i64;
        let update_sql = match kind.as_str() {
            "folder" => "UPDATE notebook_folders SET sort_order = ?2 WHERE id = ?1",
            _ => "UPDATE notebook_notes SET sort_order = ?2 WHERE id = ?1",
        };

        conn.execute(update_sql, libsql::params![id.as_str(), new_order])
            .await
            .context("Failed to renumber sibling sort_order")?;
    }

    Ok(())
}

pub async fn gather_subtree_image_public_ids(
    conn: &Connection,
    folder_id: &str,
) -> Result<Vec<String>> {
    let mut rows = conn
        .query(
            r#"
            WITH RECURSIVE subtree(id) AS (
                SELECT id FROM notebook_folders WHERE id = ?1
                UNION ALL
                SELECT f.id FROM notebook_folders f JOIN subtree s ON f.parent_folder_id = s.id
            )
            SELECT i.cloudinary_public_id FROM notebook_images i
            JOIN notebook_notes n ON i.note_id = n.id
            WHERE n.folder_id IN (SELECT id FROM subtree)
            "#,
            libsql::params![folder_id],
        )
        .await
        .context("Failed to gather subtree image public ids")?;

    let mut public_ids = Vec::new();
    while let Some(row) = rows.next().await? {
        public_ids.push(row.get::<String>(0)?);
    }

    Ok(public_ids)
}

pub async fn delete_notebook_folder_subtree(conn: &Connection, folder_id: &str) -> Result<()> {
    // The FK on notebook_notes.folder_id is NOT enforced on pre-v0.4 DBs, so
    // delete the notes explicitly BEFORE deleting folders. This cascades to
    // notebook_images + notebook_note_trades via their enforced note_id FKs.
    conn.execute(
        r#"
        DELETE FROM notebook_notes WHERE folder_id IN (
            WITH RECURSIVE subtree(id) AS (
                SELECT id FROM notebook_folders WHERE id = ?1
                UNION ALL
                SELECT f.id FROM notebook_folders f JOIN subtree s ON f.parent_folder_id = s.id
            )
            SELECT id FROM subtree
        )
        "#,
        libsql::params![folder_id],
    )
    .await
    .context("Failed to delete notes within folder subtree")?;

    // The self-referential FK on notebook_folders (new table, enforced) cascades
    // the delete to all descendant folders.
    conn.execute(
        "DELETE FROM notebook_folders WHERE id = ?1",
        libsql::params![folder_id],
    )
    .await
    .context("Failed to delete notebook folder subtree")?;

    Ok(())
}
