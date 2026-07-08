use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::accounts_table;
use super::journal_table;
use super::notebook_images::{self, NotebookImage};

const UNTITLED_NOTE_TITLE: &str = "Title";
const SELECT_COLS: &str = "id, user_id, account_id, folder_id, sort_order, title, document_json, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

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

fn row_to_notebook_note_row(row: &sqlx::postgres::PgRow) -> Result<NotebookNoteRow> {
    Ok(NotebookNoteRow {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        folder_id: row.try_get::<Option<String>, _>(3)?,
        sort_order: row.try_get::<i64, _>(4)?,
        title: row.try_get::<String, _>(5)?,
        document_json: row.try_get::<String, _>(6)?,
        created_at: row.try_get::<String, _>(7)?,
        updated_at: row.try_get::<String, _>(8)?,
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

async fn ensure_account_exists(pool: &PgPool, user_id: &str, account_id: &str) -> Result<()> {
    let account = accounts_table::find_account(pool, account_id, user_id)
        .await
        .with_context(|| format!("Failed to verify account '{account_id}'"))?;
    ensure!(account.is_some(), "Account '{account_id}' was not found");
    Ok(())
}

async fn validate_trade_ids(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    trade_ids: &[String],
) -> Result<()> {
    for trade_id in trade_ids {
        let trade = journal_table::find_journal_entry(pool, trade_id, user_id)
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
    pool: &PgPool,
    user_id: &str,
    input: CreateNotebookNoteInput,
) -> Result<PreparedNotebookNote> {
    let account_id = normalize_required_text(&input.account_id, "account_id")?;
    ensure_account_exists(pool, user_id, &account_id).await?;

    let (document_json, title) = normalize_document_json(&input.document_json)?;
    let trade_ids = normalize_trade_ids(input.trade_ids)?;
    validate_trade_ids(pool, user_id, &account_id, &trade_ids).await?;

    Ok(PreparedNotebookNote {
        account_id,
        title,
        document_json,
        trade_ids,
        folder_id: input.folder_id,
    })
}

async fn prepare_update_note(
    pool: &PgPool,
    user_id: &str,
    current: &NotebookNote,
    input: UpdateNotebookNoteInput,
) -> Result<PreparedNotebookNote> {
    let account_id = match input.account_id {
        Some(account_id) => normalize_required_text(&account_id, "account_id")?,
        None => current.account_id.clone(),
    };
    ensure_account_exists(pool, user_id, &account_id).await?;

    let (document_json, title) = match input.document_json {
        Some(document_json) => normalize_document_json(&document_json)?,
        None => (current.document_json.clone(), current.title.clone()),
    };

    let trade_ids = match input.trade_ids {
        Some(trade_ids) => normalize_trade_ids(trade_ids)?,
        None => current.trade_ids.clone(),
    };

    validate_trade_ids(pool, user_id, &account_id, &trade_ids).await?;

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
    pool: &PgPool,
    note_id: &str,
    user_id: &str,
) -> Result<Vec<String>> {
    let rows = sqlx::query(
        r#"
            SELECT nnt.trade_id
            FROM notebook_note_trades nnt
            INNER JOIN journal_entries je ON je.id = nnt.trade_id
            WHERE nnt.note_id = $1 AND je.user_id = $2
            ORDER BY nnt.created_at ASC, nnt.trade_id ASC
            "#,
    )
    .bind(note_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list note trade links")?;

    let mut trade_ids = Vec::new();
    for row in &rows {
        trade_ids.push(row.try_get::<String, _>(0)?);
    }

    Ok(trade_ids)
}

/// Batched sibling of `list_trade_ids_for_note`: fetches trade links for a set of
/// notes in one query, returning `(note_id, trade_id)` pairs. Mirrors the
/// single-note table/join/user scoping exactly, swapping `note_id = $1` for
/// `note_id = ANY($1)`. The `ORDER BY nnt.note_id ASC` groups rows by note, and
/// within each note the trailing `created_at ASC, nnt.trade_id ASC` reproduces the
/// single-note ordering.
async fn list_trade_ids_for_notes(
    pool: &PgPool,
    note_ids: &[String],
    user_id: &str,
) -> Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        r#"
            SELECT nnt.note_id, nnt.trade_id
            FROM notebook_note_trades nnt
            INNER JOIN journal_entries je ON je.id = nnt.trade_id
            WHERE nnt.note_id = ANY($1) AND je.user_id = $2
            ORDER BY nnt.note_id ASC, nnt.created_at ASC, nnt.trade_id ASC
            "#,
    )
    .bind(note_ids)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list note trade links")?;

    let mut pairs = Vec::new();
    for row in &rows {
        let note_id = row.try_get::<String, _>(0)?;
        let trade_id = row.try_get::<String, _>(1)?;
        pairs.push((note_id, trade_id));
    }

    Ok(pairs)
}

/// Pure, DB-free grouping + assembly used by `list_notebook_notes`. Takes the
/// ordered note rows and the two flat `(note_id, _)` vecs (already ordered by the
/// batched queries) and returns notes in the SAME order as `note_rows`, each with
/// its trade ids and images grouped by note. Extracted so the ordering/association
/// logic can be unit-tested without a database.
fn assemble_notebook_notes(
    note_rows: Vec<NotebookNoteRow>,
    trade_id_pairs: Vec<(String, String)>,
    image_pairs: Vec<(String, NotebookImage)>,
) -> Vec<NotebookNote> {
    let mut trade_ids_by_note: std::collections::HashMap<String, Vec<String>> =
        std::collections::HashMap::new();
    for (note_id, trade_id) in trade_id_pairs {
        trade_ids_by_note.entry(note_id).or_default().push(trade_id);
    }

    let mut images_by_note: std::collections::HashMap<String, Vec<NotebookImage>> =
        std::collections::HashMap::new();
    for (note_id, image) in image_pairs {
        images_by_note.entry(note_id).or_default().push(image);
    }

    note_rows
        .into_iter()
        .map(|note_row| {
            let trade_ids = trade_ids_by_note.remove(&note_row.id).unwrap_or_default();
            let images = images_by_note.remove(&note_row.id).unwrap_or_default();
            to_notebook_note(note_row, trade_ids, images)
        })
        .collect()
}

async fn sync_trade_links(pool: &PgPool, note_id: &str, trade_ids: &[String]) -> Result<()> {
    // Clear then re-insert the full link set in one transaction so a partial
    // failure cannot leave the note with a half-rewritten trade list.
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM notebook_note_trades WHERE note_id = $1")
        .bind(note_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear note trade links")?;

    for trade_id in trade_ids {
        sqlx::query("INSERT INTO notebook_note_trades (note_id, trade_id) VALUES ($1, $2)")
            .bind(note_id)
            .bind(trade_id.as_str())
            .execute(&mut *tx)
            .await
            .with_context(|| format!("Failed to link trade '{trade_id}' to note '{note_id}'"))?;
    }

    tx.commit().await?;

    Ok(())
}

pub async fn list_notebook_notes(
    pool: &PgPool,
    user_id: &str,
    account_id: Option<&str>,
) -> Result<Vec<NotebookNote>> {
    let rows = if let Some(account_id) = account_id {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM notebook_notes WHERE user_id = $1 AND account_id = $2 ORDER BY sort_order ASC, updated_at DESC"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(user_id)
            .bind(account_id)
            .fetch_all(pool)
            .await
    } else {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM notebook_notes WHERE user_id = $1 ORDER BY sort_order ASC, updated_at DESC"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(user_id)
            .fetch_all(pool)
            .await
    }
    .context("Failed to list notebook notes")?;

    // Materialize the note rows once (preserving the sort_order ASC, updated_at DESC
    // ordering from the query above), then fetch all trade links and images for the
    // whole set in two batched queries instead of 2N per-note round-trips. Total: 3
    // queries regardless of note count.
    let mut note_rows = Vec::with_capacity(rows.len());
    for row in &rows {
        note_rows.push(row_to_notebook_note_row(row)?);
    }

    let note_ids: Vec<String> = note_rows.iter().map(|note_row| note_row.id.clone()).collect();

    let trade_id_pairs = list_trade_ids_for_notes(pool, &note_ids, user_id).await?;
    let image_pairs =
        notebook_images::list_notebook_images_for_notes(pool, &note_ids, user_id).await?;

    Ok(assemble_notebook_notes(note_rows, trade_id_pairs, image_pairs))
}

pub async fn find_notebook_note(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<NotebookNote>> {
    let sql = format!("SELECT {SELECT_COLS} FROM notebook_notes WHERE id = $1 AND user_id = $2");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find notebook note")?;

    match row {
        Some(row) => {
            let note_row = row_to_notebook_note_row(&row)?;
            let trade_ids = list_trade_ids_for_note(pool, &note_row.id, user_id).await?;
            let images =
                notebook_images::list_notebook_images_for_note(pool, &note_row.id, user_id).await?;
            Ok(Some(to_notebook_note(note_row, trade_ids, images)))
        }
        None => Ok(None),
    }
}

pub async fn create_notebook_note(
    pool: &PgPool,
    user_id: &str,
    input: CreateNotebookNoteInput,
) -> Result<NotebookNote> {
    let prepared = prepare_create_note(pool, user_id, input).await?;
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        r#"
        INSERT INTO notebook_notes (id, user_id, account_id, folder_id, title, document_json)
        VALUES ($1, $2, $3, $4, $5, $6)
        "#,
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(prepared.account_id.as_str())
    .bind(prepared.folder_id.as_deref())
    .bind(prepared.title.as_str())
    .bind(prepared.document_json.as_str())
    .execute(pool)
    .await
    .context("Failed to insert notebook note")?;

    sync_trade_links(pool, &id, &prepared.trade_ids).await?;

    find_notebook_note(pool, &id, user_id)
        .await?
        .context("Notebook note not found after insert")
}

pub async fn update_notebook_note(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdateNotebookNoteInput,
) -> Result<NotebookNote> {
    let current = find_notebook_note(pool, id, user_id)
        .await?
        .ok_or_else(|| anyhow!("Notebook note '{id}' not found"))?;

    let prepared = prepare_update_note(pool, user_id, &current, input).await?;

    sqlx::query(
        r#"
        UPDATE notebook_notes
        SET account_id = $1, folder_id = $2, title = $3, document_json = $4
        WHERE id = $5 AND user_id = $6
        "#,
    )
    .bind(prepared.account_id.as_str())
    .bind(prepared.folder_id.as_deref())
    .bind(prepared.title.as_str())
    .bind(prepared.document_json.as_str())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update notebook note")?;

    sync_trade_links(pool, id, &prepared.trade_ids).await?;
    notebook_images::sync_note_image_account_id(pool, id, user_id, &prepared.account_id).await?;

    find_notebook_note(pool, id, user_id)
        .await?
        .context("Notebook note not found after update")
}

pub async fn delete_notebook_note(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = sqlx::query("DELETE FROM notebook_notes WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete notebook note")?
        .rows_affected();

    Ok(rows_affected > 0)
}

#[cfg(test)]
mod tests {
    use super::notebook_images::NotebookImage;
    use super::{
        NotebookNoteRow, UNTITLED_NOTE_TITLE, assemble_notebook_notes, derive_note_title,
        normalize_document_json,
    };
    use serde_json::json;

    fn make_note_row(id: &str) -> NotebookNoteRow {
        NotebookNoteRow {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            account_id: "account-1".to_string(),
            folder_id: None,
            sort_order: 0,
            title: "Title".to_string(),
            document_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    fn make_image(id: &str, note_id: &str) -> NotebookImage {
        NotebookImage {
            id: id.to_string(),
            note_id: note_id.to_string(),
            user_id: "user-1".to_string(),
            account_id: "account-1".to_string(),
            cloudinary_asset_id: String::new(),
            cloudinary_public_id: String::new(),
            secure_url: String::new(),
            width: 0,
            height: 0,
            format: String::new(),
            bytes: 0,
            original_filename: String::new(),
            media_type: String::new(),
            content_type: String::new(),
            duration_seconds: 0.0,
            created_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn assembles_notes_in_input_order_with_grouped_children() {
        // Notes arrive in the order produced by the notes query; the helper must
        // return them in that exact order with children grouped by note_id.
        let note_rows = vec![make_note_row("A"), make_note_row("B"), make_note_row("C")];

        // Flat (note_id, _) vecs as the batched queries would return them.
        let trade_id_pairs = vec![("B".to_string(), "trade-b1".to_string())];
        let image_pairs = vec![
            ("B".to_string(), make_image("img-b1", "B")),
            ("B".to_string(), make_image("img-b2", "B")),
            ("C".to_string(), make_image("img-c1", "C")),
        ];

        let notes = assemble_notebook_notes(note_rows, trade_id_pairs, image_pairs);

        // Output order matches input note order.
        assert_eq!(
            notes.iter().map(|n| n.id.as_str()).collect::<Vec<_>>(),
            vec!["A", "B", "C"]
        );

        // A: no trades, no images.
        assert!(notes[0].trade_ids.is_empty());
        assert!(notes[0].images.is_empty());

        // B: 1 trade + 2 images, images in insertion (created_at) order.
        assert_eq!(notes[1].trade_ids, vec!["trade-b1".to_string()]);
        assert_eq!(
            notes[1].images.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["img-b1", "img-b2"]
        );

        // C: 1 image, no trades.
        assert!(notes[2].trade_ids.is_empty());
        assert_eq!(
            notes[2].images.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
            vec!["img-c1"]
        );
    }

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
