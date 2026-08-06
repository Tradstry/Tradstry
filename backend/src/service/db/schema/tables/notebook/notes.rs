use anyhow::{Context, Result, anyhow, ensure};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use super::super::journal_table;
use super::super::workspaces_table;
use super::crdt;
use super::images::{self, NotebookImage};
use crate::service::notebook::document::normalize_document_json;

const SELECT_COLS: &str = "id, user_id, workspace_id, folder_id, sort_order, title, document_json, to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at, is_starred, is_pinned";

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookNote {
    pub id: String,
    pub user_id: String,
    pub workspace_id: String,
    pub folder_id: Option<String>,
    pub sort_order: i64,
    pub title: String,
    pub document_json: String,
    pub trade_ids: Vec<String>,
    pub images: Vec<NotebookImage>,
    pub created_at: String,
    pub updated_at: String,
    pub is_starred: bool,
    pub is_pinned: bool,
}

#[derive(Debug, InputObject)]
pub struct CreateNotebookNoteInput {
    /// Client-minted UUID for offline creation; the server mints one when absent
    /// so a note can be referenced before it ever reaches the server.
    pub id: Option<String>,
    pub workspace_id: String,
    pub document_json: String,
    #[graphql(default)]
    pub trade_ids: Vec<String>,
    pub folder_id: Option<String>,
}

#[derive(Debug, Default, InputObject)]
pub struct UpdateNotebookNoteInput {
    pub workspace_id: Option<String>,
    pub document_json: Option<String>,
    pub trade_ids: Option<Vec<String>>,
    pub folder_id: Option<String>,
    /// The caller's last-known `updated_at`. When set, the update only lands if
    /// the row still carries it; otherwise the write is stale and is rejected.
    pub expected_updated_at: Option<String>,
}

#[derive(Debug, Clone)]
struct PreparedNotebookNote {
    workspace_id: String,
    title: String,
    document_json: String,
    trade_ids: Vec<String>,
    folder_id: Option<String>,
}

#[derive(Debug, Clone)]
struct NotebookNoteRow {
    id: String,
    user_id: String,
    workspace_id: String,
    folder_id: Option<String>,
    sort_order: i64,
    title: String,
    document_json: String,
    created_at: String,
    updated_at: String,
    is_starred: bool,
    is_pinned: bool,
}

fn row_to_notebook_note_row(row: &sqlx::postgres::PgRow) -> Result<NotebookNoteRow> {
    Ok(NotebookNoteRow {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        workspace_id: row.try_get::<String, _>(2)?,
        folder_id: row.try_get::<Option<String>, _>(3)?,
        sort_order: row.try_get::<i64, _>(4)?,
        title: row.try_get::<String, _>(5)?,
        document_json: row.try_get::<String, _>(6)?,
        created_at: row.try_get::<String, _>(7)?,
        updated_at: row.try_get::<String, _>(8)?,
        is_starred: row.try_get::<bool, _>(9)?,
        is_pinned: row.try_get::<bool, _>(10)?,
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
        workspace_id: row.workspace_id,
        folder_id: row.folder_id,
        sort_order: row.sort_order,
        title: row.title,
        document_json: row.document_json,
        trade_ids,
        images,
        created_at: row.created_at,
        updated_at: row.updated_at,
        is_starred: row.is_starred,
        is_pinned: row.is_pinned,
    }
}

/// Set the star/pin flags on a note, bumping the HLC + `updated_at` so the change is a proper
/// last-writer edit. Either flag may be left `None` to keep its current value. Deliberately
/// does not touch title/body/folder — this is metadata only.
pub async fn set_notebook_note_flags(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    is_starred: Option<bool>,
    is_pinned: Option<bool>,
) -> Result<NotebookNote> {
    let affected = sqlx::query(
        r#"
        UPDATE notebook_notes
        SET is_starred = COALESCE($3, is_starred),
            is_pinned = COALESCE($4, is_pinned),
            hlc = $5
        WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(user_id)
    .bind(is_starred)
    .bind(is_pinned)
    .bind(crate::service::hlc::stamp())
    .execute(pool)
    .await
    .context("Failed to update notebook note flags")?
    .rows_affected();

    ensure!(affected > 0, "Notebook note '{id}' not found");

    find_notebook_note(pool, id, user_id)
        .await?
        .context("Notebook note not found after flag update")
}

fn normalize_required_text(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "{field} cannot be empty");
    Ok(trimmed.to_string())
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

async fn ensure_account_exists(
    conn: &mut PgConnection,
    user_id: &str,
    workspace_id: &str,
) -> Result<()> {
    let account = workspaces_table::find_workspace(&mut *conn, workspace_id, user_id)
        .await
        .with_context(|| format!("Failed to verify account '{workspace_id}'"))?;
    ensure!(
        account.is_some(),
        "Workspace '{workspace_id}' was not found"
    );
    Ok(())
}

async fn validate_trade_ids(
    conn: &mut PgConnection,
    user_id: &str,
    workspace_id: &str,
    trade_ids: &[String],
) -> Result<()> {
    for trade_id in trade_ids {
        let trade = journal_table::find_journal_entry(&mut *conn, trade_id, user_id)
            .await
            .with_context(|| format!("Failed to verify trade '{trade_id}'"))?
            .ok_or_else(|| anyhow!("Trade '{trade_id}' was not found"))?;

        ensure!(
            trade.workspace_id == workspace_id,
            "Trade '{trade_id}' does not belong to account '{workspace_id}'"
        );
    }

    Ok(())
}

async fn prepare_create_note(
    conn: &mut PgConnection,
    user_id: &str,
    input: CreateNotebookNoteInput,
) -> Result<PreparedNotebookNote> {
    let workspace_id = normalize_required_text(&input.workspace_id, "workspace_id")?;
    ensure_account_exists(conn, user_id, &workspace_id).await?;

    let (document_json, title) = normalize_document_json(&input.document_json)?;
    let trade_ids = normalize_trade_ids(input.trade_ids)?;
    validate_trade_ids(conn, user_id, &workspace_id, &trade_ids).await?;

    Ok(PreparedNotebookNote {
        workspace_id,
        title,
        document_json,
        trade_ids,
        folder_id: input.folder_id,
    })
}

async fn prepare_update_note(
    conn: &mut PgConnection,
    user_id: &str,
    current: &NotebookNote,
    input: UpdateNotebookNoteInput,
) -> Result<PreparedNotebookNote> {
    let workspace_id = match input.workspace_id {
        Some(workspace_id) => normalize_required_text(&workspace_id, "workspace_id")?,
        None => current.workspace_id.clone(),
    };
    ensure_account_exists(conn, user_id, &workspace_id).await?;

    let (document_json, title) = match input.document_json {
        Some(document_json) => normalize_document_json(&document_json)?,
        None => (current.document_json.clone(), current.title.clone()),
    };

    let trade_ids = match input.trade_ids {
        Some(trade_ids) => normalize_trade_ids(trade_ids)?,
        None => current.trade_ids.clone(),
    };

    validate_trade_ids(conn, user_id, &workspace_id, &trade_ids).await?;

    let folder_id = match input.folder_id {
        Some(folder_id) => Some(folder_id),
        None => current.folder_id.clone(),
    };

    Ok(PreparedNotebookNote {
        workspace_id,
        title,
        document_json,
        trade_ids,
        folder_id,
    })
}

pub(super) async fn list_trade_ids_for_note(
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

async fn sync_trade_links_conn(
    conn: &mut PgConnection,
    note_id: &str,
    trade_ids: &[String],
) -> Result<()> {
    sqlx::query("DELETE FROM notebook_note_trades WHERE note_id = $1")
        .bind(note_id)
        .execute(&mut *conn)
        .await
        .context("Failed to clear note trade links")?;

    for trade_id in trade_ids {
        sqlx::query("INSERT INTO notebook_note_trades (note_id, trade_id) VALUES ($1, $2)")
            .bind(note_id)
            .bind(trade_id.as_str())
            .execute(&mut *conn)
            .await
            .with_context(|| format!("Failed to link trade '{trade_id}' to note '{note_id}'"))?;
    }

    Ok(())
}

async fn sync_trade_links(pool: &PgPool, note_id: &str, trade_ids: &[String]) -> Result<()> {
    // Clear then re-insert the full link set in one transaction so a partial
    // failure cannot leave the note with a half-rewritten trade list.
    let mut tx = pool.begin().await?;
    sync_trade_links_conn(&mut tx, note_id, trade_ids).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn list_notebook_notes(
    pool: &PgPool,
    user_id: &str,
    workspace_id: Option<&str>,
) -> Result<Vec<NotebookNote>> {
    let rows = if let Some(workspace_id) = workspace_id {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM notebook_notes WHERE user_id = $1 AND workspace_id = $2 AND deleted_at IS NULL ORDER BY sort_order ASC, updated_at DESC"
        );
        sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(user_id)
            .bind(workspace_id)
            .fetch_all(pool)
            .await
    } else {
        let sql = format!(
            "SELECT {SELECT_COLS} FROM notebook_notes WHERE user_id = $1 AND deleted_at IS NULL ORDER BY sort_order ASC, updated_at DESC"
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

    let note_ids: Vec<String> = note_rows
        .iter()
        .map(|note_row| note_row.id.clone())
        .collect();

    let trade_id_pairs = list_trade_ids_for_notes(pool, &note_ids, user_id).await?;
    let image_pairs = images::list_notebook_images_for_notes(pool, &note_ids, user_id).await?;

    Ok(assemble_notebook_notes(
        note_rows,
        trade_id_pairs,
        image_pairs,
    ))
}

pub async fn find_notebook_note(
    pool: &PgPool,
    id: &str,
    user_id: &str,
) -> Result<Option<NotebookNote>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM notebook_notes WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL"
    );
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
            let images = images::list_notebook_images_for_note(pool, &note_row.id, user_id).await?;
            Ok(Some(to_notebook_note(note_row, trade_ids, images)))
        }
        None => Ok(None),
    }
}

/// Transactional core: inserts the note (and its trade links) on the caller's
/// connection so a sync mutation can commit the effect and its watermark in one
/// transaction. `hlc` is the last-writer stamp ("" for server-authored rows).
pub async fn create_notebook_note_tx(
    conn: &mut PgConnection,
    user_id: &str,
    input: CreateNotebookNoteInput,
    hlc: &str,
) -> Result<String> {
    let id = match input.id.as_deref() {
        Some(id) => {
            Uuid::parse_str(id).context("Client-supplied note id must be a UUID")?;
            id.to_string()
        }
        None => Uuid::new_v4().to_string(),
    };
    let prepared = prepare_create_note(conn, user_id, input).await?;

    sqlx::query(
        r#"
        INSERT INTO notebook_notes (id, user_id, workspace_id, folder_id, title, document_json, hlc)
        VALUES ($1, $2, $3, $4, $5, $6, $7)
        "#,
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(prepared.workspace_id.as_str())
    .bind(prepared.folder_id.as_deref())
    .bind(prepared.title.as_str())
    .bind(prepared.document_json.as_str())
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("Failed to insert notebook note")?;

    sync_trade_links_conn(conn, &id, &prepared.trade_ids).await?;

    Ok(id)
}

pub async fn create_notebook_note(
    pool: &PgPool,
    user_id: &str,
    input: CreateNotebookNoteInput,
) -> Result<NotebookNote> {
    let mut tx = pool.begin().await?;
    let id =
        create_notebook_note_tx(&mut tx, user_id, input, &crate::service::hlc::stamp()).await?;
    tx.commit().await?;

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

    if input.document_json.is_some() && crdt::note_state(pool, id).await? != crdt::NoteState::Legacy
    {
        anyhow::bail!("CRDT_NOTE: body writes must go through appendNotebookUpdates");
    }

    let expected_updated_at = input.expected_updated_at.clone();

    let mut conn = pool.acquire().await?;
    let prepared = prepare_update_note(&mut conn, user_id, &current, input).await?;
    drop(conn);

    // A stale write must fail rather than win: when the caller supplies its
    // last-seen updated_at, the guard makes the UPDATE a no-op if the row has
    // since moved on, so a concurrent (e.g. desktop-synced) merge is never
    // clobbered. `None` skips the guard, preserving the pre-existing behavior.
    // On a crdt note the body belongs to the projection. A metadata-only update must
    // not write back the snapshot it read, or it silently reverts a concurrent
    // refresh_projection while projected_seq still says the row is fresh.
    let is_legacy = crdt::note_state(pool, id).await? == crdt::NoteState::Legacy;

    let affected = sqlx::query(
        r#"
        UPDATE notebook_notes
        SET workspace_id = $1,
            folder_id = $2,
            title = CASE WHEN $8 THEN $3 ELSE title END,
            document_json = CASE WHEN $8 THEN $4 ELSE document_json END,
            hlc = $9
        WHERE id = $5 AND user_id = $6
          AND ($7::text IS NULL OR updated_at = $7::timestamptz)
        "#,
    )
    .bind(prepared.workspace_id.as_str())
    .bind(prepared.folder_id.as_deref())
    .bind(prepared.title.as_str())
    .bind(prepared.document_json.as_str())
    .bind(id)
    .bind(user_id)
    .bind(expected_updated_at.as_deref())
    .bind(is_legacy)
    .bind(crate::service::hlc::stamp())
    .execute(pool)
    .await
    .context("Failed to update notebook note")?
    .rows_affected();

    // The note existed and was not deleted (find_notebook_note above filters
    // deleted rows), so zero affected rows can only mean the guard failed.
    if expected_updated_at.is_some() && affected == 0 {
        return Err(anyhow!("CONFLICT: note was modified"));
    }

    sync_trade_links(pool, id, &prepared.trade_ids).await?;
    images::sync_note_image_workspace_id(pool, id, user_id, &prepared.workspace_id).await?;

    find_notebook_note(pool, id, user_id)
        .await?
        .context("Notebook note not found after update")
}

pub async fn delete_notebook_note_tx(
    conn: &mut PgConnection,
    id: &str,
    user_id: &str,
    hlc: &str,
) -> Result<bool> {
    let rows_affected = sqlx::query(
        "UPDATE notebook_notes SET deleted_at = now(), hlc = $3 \
         WHERE id = $1 AND user_id = $2 AND deleted_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("Failed to soft-delete notebook note")?
    .rows_affected();

    Ok(rows_affected > 0)
}

pub async fn delete_notebook_note(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let mut conn = pool.acquire().await?;
    delete_notebook_note_tx(&mut conn, id, user_id, &crate::service::hlc::stamp()).await
}

#[cfg(test)]
mod tests {
    use super::images::NotebookImage;
    use super::{NotebookNoteRow, assemble_notebook_notes};

    fn make_note_row(id: &str) -> NotebookNoteRow {
        NotebookNoteRow {
            id: id.to_string(),
            user_id: "user-1".to_string(),
            workspace_id: "account-1".to_string(),
            folder_id: None,
            sort_order: 0,
            title: "Title".to_string(),
            document_json: "{}".to_string(),
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
            is_starred: false,
            is_pinned: false,
        }
    }

    fn make_image(id: &str, note_id: &str) -> NotebookImage {
        NotebookImage {
            id: id.to_string(),
            note_id: note_id.to_string(),
            user_id: "user-1".to_string(),
            workspace_id: "account-1".to_string(),
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
            content_hash: String::new(),
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
            notes[1]
                .images
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["img-b1", "img-b2"]
        );

        // C: 1 image, no trades.
        assert!(notes[2].trade_ids.is_empty());
        assert_eq!(
            notes[2]
                .images
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>(),
            vec!["img-c1"]
        );
    }
}
