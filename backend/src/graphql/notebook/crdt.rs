//! The byte pipe that carries Yjs CRDT update blobs to Postgres `bytea`.
//!
//! Rust never interprets an update. Blobs are `Vec<u8>` end to end and only ever
//! become a `String` as base64 at the GraphQL wire boundary — a Yjs update
//! contains `0x00` and invalid UTF-8, so routing it through any other `String`
//! silently corrupts the document.

use anyhow::{Context as AnyhowContext, Result, bail};
use async_graphql::{Context, Object, SimpleObject};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use sqlx::PgPool;

use crate::service::db::schema::tables::notebook::crdt::{self, NoteState};

/// Rejects operations on a note the user does not own, closing cross-tenant reads
/// and appends. `notebook_note_updates` has no `user_id`; ownership lives on
/// `notebook_notes`, so the guard checks there.
async fn ensure_note_owned<'e, E>(executor: E, note_id: &str, user_id: &str) -> Result<()>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row: Option<(String,)> =
        sqlx::query_as("SELECT id FROM notebook_notes WHERE id = $1 AND user_id = $2")
            .bind(note_id)
            .bind(user_id)
            .fetch_optional(executor)
            .await
            .context("failed to verify note ownership")?;
    if row.is_none() {
        bail!("note not found");
    }
    Ok(())
}

/// Appends `updates` in order and returns the note's new max `seq`. Ownership,
/// the crdt-state guard, and every insert commit in ONE transaction: a partial
/// append or an append past an ownership/state check must never persist.
pub async fn append_updates(
    pool: &PgPool,
    user_id: &str,
    note_id: &str,
    updates: &[Vec<u8>],
) -> Result<i64> {
    let mut tx = pool.begin().await?;
    let max_seq = append_updates_tx(&mut tx, user_id, note_id, updates).await?;
    tx.commit().await?;
    Ok(max_seq)
}

/// Transactional variant, so the sync push path can append inside its own savepoint.
/// The pool version is a thin wrapper: one code path, one set of guards.
pub async fn append_updates_tx(
    conn: &mut sqlx::PgConnection,
    user_id: &str,
    note_id: &str,
    updates: &[Vec<u8>],
) -> Result<i64> {
    ensure_note_owned(&mut *conn, note_id, user_id).await?;

    // Only `crdt` notes accept updates. A `legacy` note is still web-authoritative
    // `document_json`; a `seeding` note is mid-transition. Appending to either
    // corrupts the document.
    if crdt::note_state(&mut *conn, note_id).await? != NoteState::Crdt {
        bail!("only crdt notes accept updates (note {note_id} is not crdt)");
    }

    for update in updates {
        sqlx::query("INSERT INTO notebook_note_updates (note_id, update) VALUES ($1, $2)")
            .bind(note_id)
            .bind(update)
            .execute(&mut *conn)
            .await
            .context("failed to append update")?;
    }

    let (max_seq,): (i64,) = sqlx::query_as(
        "SELECT COALESCE(MAX(seq), 0) FROM notebook_note_updates WHERE note_id = $1",
    )
    .bind(note_id)
    .fetch_one(&mut *conn)
    .await
    .context("failed to read max seq")?;
    Ok(max_seq)
}

/// Returns `(seq, update)` rows for the note with `seq > since_seq`, ordered by
/// `seq`. Scoped by owner. `seq` is a global BIGSERIAL, so it is only
/// monotonic per note — never a per-note count.
pub async fn updates_since(
    pool: &PgPool,
    user_id: &str,
    note_id: &str,
    since_seq: i64,
) -> Result<Vec<(i64, Vec<u8>)>> {
    ensure_note_owned(pool, note_id, user_id).await?;
    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT seq, update FROM notebook_note_updates
         WHERE note_id = $1 AND seq > $2 ORDER BY seq",
    )
    .bind(note_id)
    .bind(since_seq)
    .fetch_all(pool)
    .await
    .context("failed to read updates since seq")?;
    Ok(rows)
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct NotebookUpdate {
    pub seq: i64,
    /// base64 of the raw update bytes — the only place they are a `String`.
    pub update: String,
}

#[derive(Default)]
pub struct NotebookCrdtMutation;

#[Object]
impl NotebookCrdtMutation {
    async fn append_notebook_updates(
        &self,
        ctx: &Context<'_>,
        note_id: String,
        updates: Vec<String>,
    ) -> async_graphql::Result<i64> {
        let user_db = super::base::get_user_db(ctx).await?;
        let decoded: Vec<Vec<u8>> = updates
            .iter()
            .map(|u| BASE64.decode(u.trim()))
            .collect::<std::result::Result<_, _>>()
            .map_err(|e| async_graphql::Error::new(format!("invalid base64 update: {e}")))?;
        Ok(append_updates(user_db.pool(), user_db.user_id(), &note_id, &decoded).await?)
    }
}

#[derive(Default)]
pub struct NotebookCrdtQuery;

#[Object]
impl NotebookCrdtQuery {
    async fn notebook_account_updates_since(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        since_seq: i64,
    ) -> async_graphql::Result<Vec<AccountNotebookUpdate>> {
        let user_db = super::base::get_user_db(ctx).await?;
        let rows = account_updates_since(user_db.pool(), user_db.user_id(), &account_id, since_seq)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(note_id, seq, update)| AccountNotebookUpdate {
                note_id,
                seq,
                update: BASE64.encode(update),
            })
            .collect())
    }

    async fn notebook_updates_since(
        &self,
        ctx: &Context<'_>,
        note_id: String,
        since_seq: i64,
    ) -> async_graphql::Result<Vec<NotebookUpdate>> {
        let user_db = super::base::get_user_db(ctx).await?;
        let rows = updates_since(user_db.pool(), user_db.user_id(), &note_id, since_seq).await?;
        Ok(rows
            .into_iter()
            .map(|(seq, update)| NotebookUpdate {
                seq,
                update: BASE64.encode(update),
            })
            .collect())
    }
}

/// One update row, tagged with the note it belongs to.
#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct AccountNotebookUpdate {
    pub note_id: String,
    pub seq: i64,
    /// base64 of the raw update bytes — the only place they are a `String`.
    pub update: String,
}

/// Bounds one pull. A device that has been offline for a long time catches up over
/// several ticks rather than loading an unbounded result set into memory.
const ACCOUNT_UPDATES_LIMIT: i64 = 500;

/// Every update across an account's notes with `seq > since_seq`, oldest first.
///
/// `seq` is a global BIGSERIAL, so one cursor covers every note in the account and
/// a device needs a single round-trip per sync rather than one per note.
pub async fn account_updates_since(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    since_seq: i64,
) -> Result<Vec<(String, i64, Vec<u8>)>> {
    let rows: Vec<(String, i64, Vec<u8>)> = sqlx::query_as(
        "SELECT u.note_id, u.seq, u.update
         FROM notebook_note_updates u
         JOIN notebook_notes n ON n.id = u.note_id
         WHERE n.user_id = $1 AND n.account_id = $2 AND u.seq > $3
         ORDER BY u.seq
         LIMIT $4",
    )
    .bind(user_id)
    .bind(account_id)
    .bind(since_seq)
    .bind(ACCOUNT_UPDATES_LIMIT)
    .fetch_all(pool)
    .await
    .context("failed to read account updates since seq")?;
    Ok(rows)
}
