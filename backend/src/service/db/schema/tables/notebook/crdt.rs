use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use serde_json::Value;
use sqlx::PgPool;

use crate::service::ai::projector;

#[derive(Debug, PartialEq)]
pub enum NoteState {
    Legacy,
    Seeding,
    Crdt,
}

/// Generic over the executor so the pool and an in-flight transaction share one
/// code path — the sync push path must apply the same guard from inside its tx.
pub async fn note_state<'e, E>(executor: E, note_id: &str) -> Result<NoteState>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let row: Option<(String,)> =
        sqlx::query_as("SELECT state FROM notebook_note_crdt WHERE note_id = $1")
            .bind(note_id)
            .fetch_optional(executor)
            .await
            .context("failed to read note crdt state")?;

    // No row means the note never entered the CRDT lifecycle, so it is still the
    // web-authoritative `document_json` note: Legacy.
    Ok(match row.as_ref().map(|r| r.0.as_str()) {
        None => NoteState::Legacy,
        Some("seeding") => NoteState::Seeding,
        _ => NoteState::Crdt,
    })
}

/// Transition a note from `legacy` to `crdt`: seed a Y.Doc from its
/// `document_json`, store the first update row plus the real state vector, and
/// flip the flag.
///
/// Idempotent on purpose: a note already `crdt` is a no-op. This is
/// load-bearing — two Y.Docs independently seeded from the same `document_json`
/// do not conflict, they merge by concatenation, silently duplicating every
/// paragraph. Only the server seeds, exactly once.
pub async fn seed_note(pool: &PgPool, note_id: &str) -> Result<()> {
    if note_state(pool, note_id).await? == NoteState::Crdt {
        return Ok(());
    }

    // Claim `seeding` before the projector runs so plain body writes are rejected
    // (Task 2) for the ~130ms subprocess window. ON CONFLICT DO NOTHING lets a
    // note left in `seeding` by a process that died mid-seed be re-driven to
    // completion by simply calling seed_note again. The placeholder vector here
    // is transient: nothing reads a seeding note's vector, and Tx2 overwrites it.
    sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector)
         VALUES ($1, 'seeding', ''::bytea)
         ON CONFLICT (note_id) DO NOTHING",
    )
    .bind(note_id)
    .execute(pool)
    .await
    .context("failed to claim seeding state")?;

    let (document_json,): (String,) =
        sqlx::query_as("SELECT document_json FROM notebook_notes WHERE id = $1")
            .bind(note_id)
            .fetch_one(pool)
            .await
            .context("seed_note: note not found")?;

    // A Postgres transaction cannot span the subprocess call without pinning a
    // connection for ~130ms, so run the projector with no tx open and commit its
    // result in a second, fast transaction.
    let seeded = projector::seed(&document_json)
        .await
        .context("seed_note: projector failed")?;

    let mut tx = pool.begin().await.context("seed_note: begin commit tx")?;
    // FOR UPDATE serializes concurrent seeds: the loser blocks here, then sees
    // `crdt` and bails without inserting a second update row — which would itself
    // duplicate content when projected.
    let (state,): (String,) =
        sqlx::query_as("SELECT state FROM notebook_note_crdt WHERE note_id = $1 FOR UPDATE")
            .bind(note_id)
            .fetch_one(&mut *tx)
            .await
            .context("seed_note: failed to lock crdt row")?;
    if state == "crdt" {
        return Ok(());
    }
    sqlx::query("INSERT INTO notebook_note_updates (note_id, update) VALUES ($1, $2)")
        .bind(note_id)
        .bind(&seeded.update)
        .execute(&mut *tx)
        .await
        .context("seed_note: failed to insert seed update")?;
    sqlx::query(
        "UPDATE notebook_note_crdt SET state = 'crdt', state_vector = $2
         WHERE note_id = $1 AND state = 'seeding'",
    )
    .bind(note_id)
    .bind(&seeded.state_vector)
    .execute(&mut *tx)
    .await
    .context("seed_note: failed to mark crdt")?;
    tx.commit().await.context("seed_note: commit failed")?;

    Ok(())
}

/// A seed is a ~130ms subprocess. Five minutes is a >2000x margin, so a seed
/// legitimately in flight is never mistaken for abandoned even under extreme
/// DB/subprocess contention, while a note orphaned by a dead process is still
/// recovered on the next sweep.
const SEED_STALE_AFTER_SECS: i64 = 300;

/// Re-drive notes stuck in `seeding`: a process that died between claiming the row
/// and committing the flip to `crdt` leaves the note frozen, rejecting every body
/// write on every client forever. `seed_note` is resumable — the claim is
/// `ON CONFLICT DO NOTHING` and no update row is written until the final tx — so
/// re-invoking it finishes the job. Returns the number of notes recovered to `crdt`.
///
/// Staleness keys on `crdt_seeded_at`, which is stamped once when the row is
/// claimed and never touched again, so its age is exactly how long the note has sat
/// in `seeding`. No extra column is needed.
///
/// This is a callable function, deliberately NOT a background timer: the batch
/// migration must call it before it begins, so any note orphaned by a previous
/// crashed run is healed before seeding resumes. Because it runs once per
/// invocation rather than in a tight loop, a genuinely unseedable note is retried
/// at most once per batch — it cannot hot-spin — so unlike the `ai_jobs` worker
/// loop it needs no attempt cap. A per-note failure is logged and skipped so one
/// bad note never aborts the sweep.
pub async fn sweep_stale_seeding(pool: &PgPool) -> Result<usize> {
    let stale_before = Utc::now() - Duration::seconds(SEED_STALE_AFTER_SECS);
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT note_id FROM notebook_note_crdt
         WHERE state = 'seeding' AND crdt_seeded_at < $1",
    )
    .bind(stale_before)
    .fetch_all(pool)
    .await
    .context("sweep_stale_seeding: failed to list stale seeds")?;

    let mut recovered = 0usize;
    for (note_id,) in rows {
        match seed_note(pool, &note_id).await {
            Ok(()) => recovered += 1,
            Err(error) => {
                log::error!("sweep_stale_seeding: note {note_id} failed to re-seed: {error:#}");
            }
        }
    }
    Ok(recovered)
}

/// True iff the projection reflects every appended update:
/// `projected_seq >= COALESCE(MAX(seq), 0)`. A crdt note with no updates, or one
/// whose projection has caught up, is fresh. Assumes a crdt row exists (callers
/// gate on `note_state`).
pub async fn is_projection_fresh(pool: &PgPool, note_id: &str) -> Result<bool> {
    let (projected_seq, max_seq): (i64, i64) = sqlx::query_as(
        "SELECT c.projected_seq,
                COALESCE((SELECT MAX(seq) FROM notebook_note_updates WHERE note_id = c.note_id), 0)
         FROM notebook_note_crdt c
         WHERE c.note_id = $1",
    )
    .bind(note_id)
    .fetch_one(pool)
    .await
    .context("failed to check projection freshness")?;
    Ok(projected_seq >= max_seq)
}

/// The update seq the projection was last built from — the value to stamp onto a
/// vector's `body_version`.
pub async fn projected_seq(pool: &PgPool, note_id: &str) -> Result<i64> {
    let (seq,): (i64,) =
        sqlx::query_as("SELECT projected_seq FROM notebook_note_crdt WHERE note_id = $1")
            .bind(note_id)
            .fetch_one(pool)
            .await
            .context("failed to read projected_seq")?;
    Ok(seq)
}

/// Rebuild `notebook_notes.document_json` (and its derived title) from the note's
/// updates, advancing `projected_seq`/`projected_at`. This is the lazy write of a
/// CRDT-derived projection.
pub async fn refresh_projection(pool: &PgPool, note_id: &str) -> Result<()> {
    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT seq, update FROM notebook_note_updates WHERE note_id = $1 ORDER BY seq",
    )
    .bind(note_id)
    .fetch_all(pool)
    .await
    .context("refresh_projection: failed to read updates")?;
    // No updates means nothing to project (projector rejects empty input) and the
    // note is already fresh at projected_seq 0.
    let Some(&(max_seq, _)) = rows.last() else {
        return Ok(());
    };

    // Record the max seq we actually projected, never a MAX(seq) re-read afterward:
    // an append landing after the SELECT must keep the note stale so the next
    // reindex catches it, rather than being marked fresh but unprojected.
    let updates: Vec<Vec<u8>> = rows.into_iter().map(|(_, update)| update).collect();

    // The projector subprocess (~130ms) runs with no transaction open, exactly as
    // seed_note does, so it never pins a connection.
    let projected = projector::project(&updates)
        .await
        .context("refresh_projection: projector failed")?;
    let parsed: Value = serde_json::from_str(&projected)
        .context("refresh_projection: projector emitted invalid JSON")?;
    let title = crate::service::notebook::document::derive_note_title(&parsed);
    let document_json = serde_json::to_string(&parsed)
        .context("refresh_projection: failed to serialize document")?;

    let mut tx = pool.begin().await.context("refresh_projection: begin tx")?;
    // FOR UPDATE serializes concurrent refreshes: an older refresh that read fewer
    // updates must not regress a newer projection's document_json or projected_seq.
    let (current_seq,): (i64,) = sqlx::query_as(
        "SELECT projected_seq FROM notebook_note_crdt WHERE note_id = $1 FOR UPDATE",
    )
    .bind(note_id)
    .fetch_one(&mut *tx)
    .await
    .context("refresh_projection: failed to lock crdt row")?;
    if current_seq >= max_seq {
        return Ok(());
    }

    // The projection is a server-derived write, not a client body edit, so it must
    // NOT route through the CRDT_NOTE-guarded update paths — it writes document_json
    // directly, keyed by note id, which is the projection's own private door.
    sqlx::query("UPDATE notebook_notes SET document_json = $1, title = $2 WHERE id = $3")
        .bind(&document_json)
        .bind(&title)
        .bind(note_id)
        .execute(&mut *tx)
        .await
        .context("refresh_projection: failed to write document_json")?;
    sqlx::query(
        "UPDATE notebook_note_crdt SET projected_seq = $2, projected_at = now() WHERE note_id = $1",
    )
    .bind(note_id)
    .bind(max_seq)
    .execute(&mut *tx)
    .await
    .context("refresh_projection: failed to advance projected_seq")?;
    tx.commit()
        .await
        .context("refresh_projection: commit failed")?;
    Ok(())
}

/// Test/seeder helper. Real seeding goes through the projector.
pub async fn mark_crdt(pool: &PgPool, note_id: &str, state_vector: &[u8]) -> Result<()> {
    sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector)
         VALUES ($1, 'crdt', $2)
         ON CONFLICT (note_id) DO UPDATE SET state = 'crdt', state_vector = EXCLUDED.state_vector",
    )
    .bind(note_id)
    .bind(state_vector)
    .execute(pool)
    .await
    .context("failed to mark note crdt")?;
    Ok(())
}

/// Compact a note once its update chain exceeds this many rows.
const COMPACT_AFTER: i64 = 200;

/// Replaces a note's update chain with one equivalent blob.
///
/// Deleting history is safe only because the merged blob supersedes exactly what
/// is deleted: we snapshot `max_seq`, merge every row `seq <= max_seq`, delete
/// only those rows, and append the merge — which lands at a *higher* seq. A client
/// polling from any cursor below `max_seq` therefore still receives a blob that
/// carries the whole history.
///
/// A concurrent append that commits at a seq above the snapshot is untouched, and
/// order does not matter: Yjs updates commute, so `[new_delta, merged]` projects
/// identically to `[merged, new_delta]`.
pub async fn compact_note(pool: &PgPool, note_id: &str) -> Result<bool> {
    if note_state(pool, note_id).await? != NoteState::Crdt {
        return Ok(false);
    }

    let rows: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT seq, update FROM notebook_note_updates WHERE note_id = $1 ORDER BY seq",
    )
    .bind(note_id)
    .fetch_all(pool)
    .await
    .context("compact_note: failed to read update chain")?;

    if (rows.len() as i64) <= COMPACT_AFTER {
        return Ok(false);
    }

    let max_seq = rows
        .last()
        .expect("non-empty: length exceeds COMPACT_AFTER")
        .0;
    let updates: Vec<Vec<u8>> = rows.into_iter().map(|(_, update)| update).collect();

    // Outside any transaction: the compactor is a subprocess.
    let merged = projector::compact(&updates)
        .await
        .context("compact_note: compactor failed")?;

    let mut tx = pool.begin().await.context("compact_note: begin tx")?;
    // Serializes against a concurrent compaction of the same note, which would
    // otherwise delete the rows this one just merged.
    sqlx::query("SELECT state FROM notebook_note_crdt WHERE note_id = $1 FOR UPDATE")
        .bind(note_id)
        .fetch_one(&mut *tx)
        .await
        .context("compact_note: failed to lock crdt row")?;

    let deleted = sqlx::query("DELETE FROM notebook_note_updates WHERE note_id = $1 AND seq <= $2")
        .bind(note_id)
        .bind(max_seq)
        .execute(&mut *tx)
        .await
        .context("compact_note: failed to delete compacted rows")?
        .rows_affected();

    // A no-op delete means another compaction already claimed this window; the
    // merged blob would then be a duplicate of history, so do not append it.
    if deleted == 0 {
        tx.rollback().await.ok();
        return Ok(false);
    }

    sqlx::query("INSERT INTO notebook_note_updates (note_id, update) VALUES ($1, $2)")
        .bind(note_id)
        .bind(&merged)
        .execute(&mut *tx)
        .await
        .context("compact_note: failed to append merged update")?;

    tx.commit().await.context("compact_note: commit failed")?;
    Ok(true)
}

/// Compacts every note whose chain has outgrown `COMPACT_AFTER`. Runs off the
/// write path, so a user's keystroke never waits on the compactor. A per-note
/// failure is logged and skipped: one unmergeable note must not stall the sweep.
pub async fn sweep_compaction(pool: &PgPool) -> Result<usize> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT note_id FROM notebook_note_updates
         GROUP BY note_id HAVING COUNT(*) > $1",
    )
    .bind(COMPACT_AFTER)
    .fetch_all(pool)
    .await
    .context("sweep_compaction: failed to list oversized notes")?;

    let mut compacted = 0usize;
    for (note_id,) in rows {
        match compact_note(pool, &note_id).await {
            Ok(true) => compacted += 1,
            Ok(false) => {}
            Err(error) => log::error!("sweep_compaction: note {note_id} failed: {error:#}"),
        }
    }
    Ok(compacted)
}

/// Installs a seed minted by the note's creator, in the caller's transaction.
///
/// The desktop creates notes offline, so it — not the server — mints their first
/// Yjs update. A note must be seeded exactly once: two independently seeded Y.Docs
/// concatenate rather than merge, silently duplicating every paragraph.
///
/// Once-ness is really enforced by the primary key on `notebook_notes`: a replayed
/// `createNote` fails its INSERT and the caller's savepoint rolls this seed back
/// with it. The `ON CONFLICT` guard below is defense in depth, so that a future
/// caller which tolerates a duplicate note cannot silently append a second seed.
pub async fn install_seed_tx(
    conn: &mut sqlx::PgConnection,
    note_id: &str,
    update: &[u8],
    state_vector: &[u8],
) -> Result<()> {
    let inserted = sqlx::query(
        "INSERT INTO notebook_note_crdt (note_id, state, state_vector)
         VALUES ($1, 'crdt', $2)
         ON CONFLICT (note_id) DO NOTHING",
    )
    .bind(note_id)
    .bind(state_vector)
    .execute(&mut *conn)
    .await
    .context("install_seed_tx: failed to insert crdt row")?
    .rows_affected();

    if inserted == 0 {
        return Ok(());
    }

    sqlx::query("INSERT INTO notebook_note_updates (note_id, update) VALUES ($1, $2)")
        .bind(note_id)
        .bind(update)
        .execute(&mut *conn)
        .await
        .context("install_seed_tx: failed to insert seed update")?;
    Ok(())
}

/// Re-projects every crdt note whose `document_json` lags its update log.
///
/// `notebook_notes.document_json` and `title` are a projection of the Yjs updates,
/// and the only other caller of `refresh_projection` is the AI reindex job — which
/// nothing enqueues for a note created through the desktop's sync push. Without this
/// sweep a desktop-authored note reads "Untitled" with no preview on every client,
/// forever, even though its body syncs correctly.
///
/// `refresh_projection` re-checks `projected_seq` under `FOR UPDATE` and no-ops when
/// already fresh, so racing the reindex job is safe. A per-note failure is logged and
/// skipped: one unprojectable note must not stall the sweep.
pub async fn sweep_projections(pool: &PgPool) -> Result<usize> {
    let note_ids = stale_projection_note_ids(pool).await?;

    let mut refreshed = 0usize;
    for note_id in note_ids {
        match refresh_projection(pool, &note_id).await {
            Ok(()) => refreshed += 1,
            Err(error) => log::error!("sweep_projections: note {note_id} failed: {error:#}"),
        }
    }
    Ok(refreshed)
}

/// Crdt notes whose `document_json` lags their update log. Split out from the sweep
/// so it can be asserted without running a database-wide mutation.
pub async fn stale_projection_note_ids(pool: &PgPool) -> Result<Vec<String>> {
    let rows: Vec<(String,)> = sqlx::query_as(
        "SELECT c.note_id FROM notebook_note_crdt c
         WHERE c.state = 'crdt'
           AND c.projected_seq < COALESCE(
                 (SELECT MAX(seq) FROM notebook_note_updates WHERE note_id = c.note_id), 0)",
    )
    .fetch_all(pool)
    .await
    .context("sweep_projections: failed to list stale projections")?;
    Ok(rows.into_iter().map(|(id,)| id).collect())
}
