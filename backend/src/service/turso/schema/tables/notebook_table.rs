use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::{InputObject, SimpleObject};
use libsql::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use super::accounts_table;
use super::journal_table;
use super::notebook_images::{self, NotebookImage};

const UNTITLED_NOTE_TITLE: &str = "Title";
const SELECT_COLS: &str =
    "id, user_id, account_id, folder_id, sort_order, title, document_json, created_at, updated_at";

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookNote {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub folder_id: Option<String>,
    pub sort_order: i64,
    pub title: String,
    pub document_json: String,
    pub trade_ids: Vec<String>,
    pub images: Vec<NotebookImage>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
pub struct CreateNotebookNoteInput {
    pub account_id: String,
    pub document_json: String,
    #[graphql(default)]
    pub trade_ids: Vec<String>,
    pub folder_id: Option<String>,
}

#[derive(Debug, InputObject)]
pub struct UpdateNotebookNoteInput {
    pub account_id: Option<String>,
    pub document_json: Option<String>,
    pub trade_ids: Option<Vec<String>>,
    pub folder_id: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedNotebookNote {
    account_id: String,
    title: String,
    document_json: String,
    trade_ids: Vec<String>,
    folder_id: Option<String>,
}

#[derive(Debug, Clone)]
struct NotebookNoteRow {
    id: String,
    user_id: String,
    account_id: String,
    folder_id: Option<String>,
    sort_order: i64,
    title: String,
    document_json: String,
    created_at: String,
    updated_at: String,
}

fn row_to_notebook_note_row(row: &libsql::Row) -> Result<NotebookNoteRow> {
    Ok(NotebookNoteRow {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        folder_id: row.get::<Option<String>>(3)?,
        sort_order: row.get::<i64>(4)?,
        title: row.get::<String>(5)?,
        document_json: row.get::<String>(6)?,
        created_at: row.get::<String>(7)?,
        updated_at: row.get::<String>(8)?,
    })
}

fn to_notebook_note(
    row: NotebookNoteRow,
    trade_ids: Vec<String>,
    images: Vec<NotebookImage>,
) -> NotebookNote {
    NotebookNote {
        id: row.id,
        user_id: row.user_id,
        account_id: row.account_id,
        folder_id: row.folder_id,
        sort_order: row.sort_order,
        title: row.title,
        document_json: row.document_json,
        trade_ids,
        images,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn normalize_required_text(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "{field} cannot be empty");
    Ok(trimmed.to_string())
}

fn collect_text(node: &Value, output: &mut String) {
    if let Some(text) = node.get("text").and_then(Value::as_str) {
        output.push_str(text);
    }

    if let Some(children) = node.get("children").and_then(Value::as_array) {
        for child in children {
            collect_text(child, output);
        }
    }
}

fn extract_node_text(node: &Value) -> Option<String> {
    let mut text = String::new();
    collect_text(node, &mut text);
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn derive_note_title(document: &Value) -> String {
    let children = document
        .get("root")
        .and_then(|root| root.get("children"))
        .and_then(Value::as_array);

    if let Some(children) = children {
        for child in children {
            let is_h1 = child.get("type").and_then(Value::as_str) == Some("heading")
                && child.get("tag").and_then(Value::as_str) == Some("h1");
            if is_h1 && let Some(title) = extract_node_text(child) {
                return title;
            }
        }

        for child in children {
            if let Some(title) = extract_node_text(child) {
                return title;
            }
        }
    }

    UNTITLED_NOTE_TITLE.to_string()
}

fn normalize_document_json(document_json: &str) -> Result<(String, String)> {
    let trimmed = document_json.trim();
    ensure!(!trimmed.is_empty(), "document_json cannot be empty");

    let parsed: Value =
        serde_json::from_str(trimmed).context("document_json must be valid JSON")?;
    ensure!(
        parsed.get("root").is_some(),
        "document_json must contain a root node"
    );

    let normalized = serde_json::to_string(&parsed).context("Failed to serialize document_json")?;
    let title = derive_note_title(&parsed);

    Ok((normalized, title))
}

fn normalize_trade_ids(trade_ids: Vec<String>) -> Result<Vec<String>> {
    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for trade_id in trade_ids {
        let normalized = normalize_required_text(&trade_id, "trade_id")?;
        if seen.insert(normalized.clone()) {
            deduped.push(normalized);
        }
    }

    Ok(deduped)
}

async fn ensure_account_exists(conn: &Connection, user_id: &str, account_id: &str) -> Result<()> {
    let account = accounts_table::find_account(conn, account_id, user_id)
        .await
        .with_context(|| format!("Failed to verify account '{account_id}'"))?;
    ensure!(account.is_some(), "Account '{account_id}' was not found");
    Ok(())
}

async fn validate_trade_ids(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    trade_ids: &[String],
) -> Result<()> {
    for trade_id in trade_ids {
        let trade = journal_table::find_journal_entry(conn, trade_id, user_id)
            .await
            .with_context(|| format!("Failed to verify trade '{trade_id}'"))?
            .ok_or_else(|| anyhow!("Trade '{trade_id}' was not found"))?;

        ensure!(
            trade.account_id == account_id,
            "Trade '{trade_id}' does not belong to account '{account_id}'"
        );
    }

    Ok(())
}

async fn prepare_create_note(
    conn: &Connection,
    user_id: &str,
    input: CreateNotebookNoteInput,
) -> Result<PreparedNotebookNote> {
    let account_id = normalize_required_text(&input.account_id, "account_id")?;
    ensure_account_exists(conn, user_id, &account_id).await?;

    let (document_json, title) = normalize_document_json(&input.document_json)?;
    let trade_ids = normalize_trade_ids(input.trade_ids)?;
    validate_trade_ids(conn, user_id, &account_id, &trade_ids).await?;

    Ok(PreparedNotebookNote {
        account_id,
        title,
        document_json,
        trade_ids,
        folder_id: input.folder_id,
    })
}

async fn prepare_update_note(
    conn: &Connection,
    user_id: &str,
    current: &NotebookNote,
    input: UpdateNotebookNoteInput,
) -> Result<PreparedNotebookNote> {
    let account_id = match input.account_id {
        Some(account_id) => normalize_required_text(&account_id, "account_id")?,
        None => current.account_id.clone(),
    };
    ensure_account_exists(conn, user_id, &account_id).await?;

    let (document_json, title) = match input.document_json {
        Some(document_json) => normalize_document_json(&document_json)?,
        None => (current.document_json.clone(), current.title.clone()),
    };

    let trade_ids = match input.trade_ids {
        Some(trade_ids) => normalize_trade_ids(trade_ids)?,
        None => current.trade_ids.clone(),
    };

    validate_trade_ids(conn, user_id, &account_id, &trade_ids).await?;

    let folder_id = match input.folder_id {
        Some(folder_id) => Some(folder_id),
        None => current.folder_id.clone(),
    };

    Ok(PreparedNotebookNote {
        account_id,
        title,
        document_json,
        trade_ids,
        folder_id,
    })
}

async fn list_trade_ids_for_note(
    conn: &Connection,
    note_id: &str,
    user_id: &str,
) -> Result<Vec<String>> {
    let mut rows = conn
        .query(
            r#"
            SELECT nnt.trade_id
            FROM notebook_note_trades nnt
            INNER JOIN journal_entries je ON je.id = nnt.trade_id
            WHERE nnt.note_id = ?1 AND je.user_id = ?2
            ORDER BY nnt.created_at ASC, nnt.trade_id ASC
            "#,
            libsql::params![note_id, user_id],
        )
        .await
        .context("Failed to list note trade links")?;

    let mut trade_ids = Vec::new();
    while let Some(row) = rows.next().await? {
        trade_ids.push(row.get::<String>(0)?);
    }

    Ok(trade_ids)
}

async fn sync_trade_links(conn: &Connection, note_id: &str, trade_ids: &[String]) -> Result<()> {
    conn.execute(
        "DELETE FROM notebook_note_trades WHERE note_id = ?1",
        libsql::params![note_id],
    )
    .await
    .context("Failed to clear note trade links")?;

    for trade_id in trade_ids {
        conn.execute(
            "INSERT INTO notebook_note_trades (note_id, trade_id) VALUES (?1, ?2)",
            libsql::params![note_id, trade_id.as_str()],
        )
        .await
        .with_context(|| format!("Failed to link trade '{trade_id}' to note '{note_id}'"))?;
    }

    Ok(())
}

pub async fn list_notebook_notes(
    conn: &Connection,
    user_id: &str,
    account_id: Option<&str>,
) -> Result<Vec<NotebookNote>> {
    let (sql, params) = if let Some(account_id) = account_id {
        (
            format!(
                "SELECT {SELECT_COLS} FROM notebook_notes WHERE user_id = ?1 AND account_id = ?2 ORDER BY sort_order ASC, updated_at DESC"
            ),
            vec![
                libsql::Value::Text(user_id.to_string()),
                libsql::Value::Text(account_id.to_string()),
            ],
        )
    } else {
        (
            format!(
                "SELECT {SELECT_COLS} FROM notebook_notes WHERE user_id = ?1 ORDER BY sort_order ASC, updated_at DESC"
            ),
            vec![libsql::Value::Text(user_id.to_string())],
        )
    };

    let mut rows = conn
        .query(&sql, libsql::params_from_iter(params))
        .await
        .context("Failed to list notebook notes")?;

    let mut notes = Vec::new();
    while let Some(row) = rows.next().await? {
        let note_row = row_to_notebook_note_row(&row)?;
        let trade_ids = list_trade_ids_for_note(conn, &note_row.id, user_id).await?;
        let images =
            notebook_images::list_notebook_images_for_note(conn, &note_row.id, user_id).await?;
        notes.push(to_notebook_note(note_row, trade_ids, images));
    }

    Ok(notes)
}

pub async fn find_notebook_note(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<NotebookNote>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM notebook_notes WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find notebook note")?;

    match rows.next().await? {
        Some(row) => {
            let note_row = row_to_notebook_note_row(&row)?;
            let trade_ids = list_trade_ids_for_note(conn, &note_row.id, user_id).await?;
            let images =
                notebook_images::list_notebook_images_for_note(conn, &note_row.id, user_id).await?;
            Ok(Some(to_notebook_note(note_row, trade_ids, images)))
        }
        None => Ok(None),
    }
}

pub async fn create_notebook_note(
    conn: &Connection,
    user_id: &str,
    input: CreateNotebookNoteInput,
) -> Result<NotebookNote> {
    let prepared = prepare_create_note(conn, user_id, input).await?;
    let id = Uuid::new_v4().to_string();

    conn.execute(
        r#"
        INSERT INTO notebook_notes (id, user_id, account_id, folder_id, title, document_json)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6)
        "#,
        libsql::params![
            id.as_str(),
            user_id,
            prepared.account_id.as_str(),
            prepared.folder_id.as_deref(),
            prepared.title.as_str(),
            prepared.document_json.as_str(),
        ],
    )
    .await
    .context("Failed to insert notebook note")?;

    sync_trade_links(conn, &id, &prepared.trade_ids).await?;

    find_notebook_note(conn, &id, user_id)
        .await?
        .context("Notebook note not found after insert")
}

pub async fn update_notebook_note(
    conn: &Connection,
    id: &str,
    user_id: &str,
    input: UpdateNotebookNoteInput,
) -> Result<NotebookNote> {
    let current = find_notebook_note(conn, id, user_id)
        .await?
        .ok_or_else(|| anyhow!("Notebook note '{id}' not found"))?;

    let prepared = prepare_update_note(conn, user_id, &current, input).await?;

    conn.execute(
        r#"
        UPDATE notebook_notes
        SET account_id = ?1, folder_id = ?2, title = ?3, document_json = ?4
        WHERE id = ?5 AND user_id = ?6
        "#,
        libsql::params![
            prepared.account_id.as_str(),
            prepared.folder_id.as_deref(),
            prepared.title.as_str(),
            prepared.document_json.as_str(),
            id,
            user_id,
        ],
    )
    .await
    .context("Failed to update notebook note")?;

    sync_trade_links(conn, id, &prepared.trade_ids).await?;
    notebook_images::sync_note_image_account_id(conn, id, user_id, &prepared.account_id).await?;

    find_notebook_note(conn, id, user_id)
        .await?
        .context("Notebook note not found after update")
}

pub async fn delete_notebook_note(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM notebook_notes WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete notebook note")?;

    Ok(rows_affected > 0)
}

#[cfg(test)]
mod tests {
    use super::{UNTITLED_NOTE_TITLE, derive_note_title, normalize_document_json};
    use serde_json::json;

    #[test]
    fn derives_title_from_first_h1_heading() {
        let document = json!({
            "root": {
                "type": "root",
                "children": [
                    {
                        "type": "heading",
                        "tag": "h1",
                        "children": [
                            {
                                "type": "text",
                                "text": "My note header"
                            }
                        ]
                    },
                    {
                        "type": "paragraph",
                        "children": []
                    }
                ]
            }
        });

        assert_eq!(derive_note_title(&document), "My note header");
    }

    #[test]
    fn falls_back_to_untitled_note_when_header_is_empty() {
        let document = json!({
            "root": {
                "type": "root",
                "children": [
                    {
                        "type": "heading",
                        "tag": "h1",
                        "children": []
                    }
                ]
            }
        });

        assert_eq!(derive_note_title(&document), UNTITLED_NOTE_TITLE);
    }

    #[test]
    fn normalizes_valid_document_json() {
        let (document_json, title) = normalize_document_json(
            r#"{"root":{"type":"root","children":[{"type":"heading","tag":"h1","children":[{"type":"text","text":"Title"}]}]}}"#,
        )
        .expect("document should normalize");

        assert!(document_json.contains("\"root\""));
        assert_eq!(title, "Title");
    }
}
