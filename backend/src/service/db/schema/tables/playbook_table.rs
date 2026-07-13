use anyhow::{Context, Result, ensure};
use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct Playbook {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CreatePlaybookInput {
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UpdatePlaybookInput {
    pub name: Option<String>,
    pub edge_name: Option<String>,
    pub entry_rules: Option<String>,
    pub exit_rules: Option<String>,
    pub position_sizing_rules: Option<String>,
    pub additional_rules: Option<String>,
    #[graphql(default)]
    pub clear_additional_rules: bool,
}

#[derive(Debug, Clone)]
struct PreparedPlaybook {
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
}

const SELECT_COLS: &str = "id, user_id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn normalize_required_text(value: &str, field: &str) -> Result<String> {
    let trimmed = value.trim();
    ensure!(!trimmed.is_empty(), "{field} cannot be empty");
    Ok(trimmed.to_string())
}

fn normalize_optional_text(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

fn row_to_playbook(row: &sqlx::postgres::PgRow) -> Result<Playbook> {
    Ok(Playbook {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        name: row.try_get::<String, _>(2)?,
        edge_name: row.try_get::<String, _>(3)?,
        entry_rules: row.try_get::<String, _>(4)?,
        exit_rules: row.try_get::<String, _>(5)?,
        position_sizing_rules: row.try_get::<String, _>(6)?,
        additional_rules: row.try_get::<Option<String>, _>(7)?,
        created_at: row.try_get::<String, _>(8)?,
        updated_at: row.try_get::<String, _>(9)?,
    })
}

fn ensure_text<T: AsRef<str>>(value: &Option<T>) -> bool {
    match value {
        Some(value) => !value.as_ref().trim().is_empty(),
        None => false,
    }
}

async fn prepare_new_playbook(input: CreatePlaybookInput) -> Result<PreparedPlaybook> {
    ensure!(
        input.name.len() <= 80,
        "playbook name must be 80 characters or less"
    );

    Ok(PreparedPlaybook {
        name: normalize_required_text(&input.name, "name")?,
        edge_name: normalize_required_text(&input.edge_name, "edge_name")?,
        entry_rules: normalize_required_text(&input.entry_rules, "entry_rules")?,
        exit_rules: normalize_required_text(&input.exit_rules, "exit_rules")?,
        position_sizing_rules: normalize_required_text(
            &input.position_sizing_rules,
            "position_sizing_rules",
        )?,
        additional_rules: if ensure_text(&input.additional_rules) {
            normalize_optional_text(input.additional_rules)
        } else {
            None
        },
    })
}

async fn prepare_updated_playbook(
    current: &Playbook,
    input: UpdatePlaybookInput,
) -> Result<PreparedPlaybook> {
    let additional_rules = if input.clear_additional_rules {
        None
    } else if input.additional_rules.is_some() {
        normalize_optional_text(input.additional_rules)
    } else {
        current.additional_rules.clone()
    };

    Ok(PreparedPlaybook {
        name: if let Some(name) = input.name {
            normalize_required_text(&name, "name")?
        } else {
            current.name.clone()
        },
        edge_name: if let Some(edge_name) = input.edge_name {
            normalize_required_text(&edge_name, "edge_name")?
        } else {
            current.edge_name.clone()
        },
        entry_rules: if let Some(entry_rules) = input.entry_rules {
            normalize_required_text(&entry_rules, "entry_rules")?
        } else {
            current.entry_rules.clone()
        },
        exit_rules: if let Some(exit_rules) = input.exit_rules {
            normalize_required_text(&exit_rules, "exit_rules")?
        } else {
            current.exit_rules.clone()
        },
        position_sizing_rules: if let Some(position_sizing_rules) = input.position_sizing_rules {
            normalize_required_text(&position_sizing_rules, "position_sizing_rules")?
        } else {
            current.position_sizing_rules.clone()
        },
        additional_rules,
    })
}

pub async fn list_playbooks(pool: &PgPool, user_id: &str) -> Result<Vec<Playbook>> {
    let sql =
        format!("SELECT {SELECT_COLS} FROM playbooks WHERE user_id = $1 ORDER BY created_at DESC");
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .fetch_all(pool)
        .await
        .context("Failed to list playbooks")?;

    let mut playbooks = Vec::new();
    for row in &rows {
        playbooks.push(row_to_playbook(row)?);
    }

    Ok(playbooks)
}

pub async fn find_playbook(pool: &PgPool, id: &str, user_id: &str) -> Result<Option<Playbook>> {
    let sql = format!("SELECT {SELECT_COLS} FROM playbooks WHERE id = $1 AND user_id = $2");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find playbook")?;

    match row {
        Some(row) => Ok(Some(row_to_playbook(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_playbook(
    pool: &PgPool,
    user_id: &str,
    input: CreatePlaybookInput,
) -> Result<Playbook> {
    let prepared = prepare_new_playbook(input).await?;
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO playbooks (id, user_id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(prepared.name.as_str())
    .bind(prepared.edge_name.as_str())
    .bind(prepared.entry_rules.as_str())
    .bind(prepared.exit_rules.as_str())
    .bind(prepared.position_sizing_rules.as_str())
    .bind(prepared.additional_rules.as_deref())
    .execute(pool)
    .await
    .context("Failed to insert playbook")?;

    find_playbook(pool, &id, user_id)
        .await?
        .context("Playbook not found after insert")
}

pub async fn update_playbook(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    input: UpdatePlaybookInput,
) -> Result<Playbook> {
    let current = find_playbook(pool, id, user_id)
        .await?
        .context("Playbook not found")?;
    let prepared = prepare_updated_playbook(&current, input).await?;

    sqlx::query(
        "UPDATE playbooks SET name = $1, edge_name = $2, entry_rules = $3, exit_rules = $4, position_sizing_rules = $5, additional_rules = $6, updated_at = now() WHERE id = $7 AND user_id = $8",
    )
    .bind(prepared.name.as_str())
    .bind(prepared.edge_name.as_str())
    .bind(prepared.entry_rules.as_str())
    .bind(prepared.exit_rules.as_str())
    .bind(prepared.position_sizing_rules.as_str())
    .bind(prepared.additional_rules.as_deref())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to update playbook")?;

    find_playbook(pool, id, user_id)
        .await?
        .context("Playbook not found after update")
}

/// `trading_principles.playbook_id` is `ON DELETE RESTRICT`, so Postgres would
/// reject this with an opaque constraint error. Check first so the caller gets
/// the blocking principle titles and can offer to reassign or remove them.
pub async fn delete_playbook(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let blocking: Vec<String> = sqlx::query_scalar(
        "SELECT title FROM trading_principles WHERE playbook_id = $1 AND user_id = $2 ORDER BY priority DESC",
    )
    .bind(id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to check principles blocking playbook deletion")?;

    ensure!(
        blocking.is_empty(),
        "cannot delete playbook: {} principle(s) depend on it: {}",
        blocking.len(),
        blocking.join(", ")
    );

    let rows_affected = sqlx::query("DELETE FROM playbooks WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete playbook")?
        .rows_affected();

    Ok(rows_affected > 0)
}

// ---- Offline-first sync (whole-row LWW + soft-delete) --------------------

/// The editable payload a `createPlaybook`/`updatePlaybook` mutation carries. The
/// server is a dumb last-writer: it writes these fields verbatim + the client's
/// `hlc`; all conflict resolution is client-side.
pub struct PlaybookWriteArgs {
    pub id: String,
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PlaybookDelta {
    pub id: String,
    pub name: String,
    pub edge_name: String,
    pub entry_rules: String,
    pub exit_rules: String,
    pub position_sizing_rules: String,
    pub additional_rules: Option<String>,
    pub hlc: String,
    pub deleted_at: Option<String>,
    pub updated_at: String,
}

const DELTA_COLS: &str = "id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules, hlc, \
    to_char(deleted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS deleted_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS updated_at";

pub async fn create_playbook_tx(
    conn: &mut PgConnection,
    user_id: &str,
    args: &PlaybookWriteArgs,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO playbooks (id, user_id, name, edge_name, entry_rules, exit_rules, position_sizing_rules, additional_rules, hlc) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (id) DO NOTHING",
    )
    .bind(&args.id)
    .bind(user_id)
    .bind(&args.name)
    .bind(&args.edge_name)
    .bind(&args.entry_rules)
    .bind(&args.exit_rules)
    .bind(&args.position_sizing_rules)
    .bind(args.additional_rules.as_deref())
    .bind(hlc)
    .execute(&mut *conn)
    .await
    .context("create_playbook_tx")?;
    Ok(())
}

pub async fn update_playbook_tx(
    conn: &mut PgConnection,
    user_id: &str,
    args: &PlaybookWriteArgs,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE playbooks SET name = $1, edge_name = $2, entry_rules = $3, exit_rules = $4, \
         position_sizing_rules = $5, additional_rules = $6, hlc = $7, updated_at = now() \
         WHERE id = $8 AND user_id = $9",
    )
    .bind(&args.name)
    .bind(&args.edge_name)
    .bind(&args.entry_rules)
    .bind(&args.exit_rules)
    .bind(&args.position_sizing_rules)
    .bind(args.additional_rules.as_deref())
    .bind(hlc)
    .bind(&args.id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .context("update_playbook_tx")?;
    Ok(())
}

pub async fn soft_delete_playbook_tx(
    conn: &mut PgConnection,
    user_id: &str,
    id: &str,
    hlc: &str,
) -> Result<()> {
    sqlx::query(
        "UPDATE playbooks SET deleted_at = now(), hlc = $1 \
         WHERE id = $2 AND user_id = $3 AND deleted_at IS NULL",
    )
    .bind(hlc)
    .bind(id)
    .bind(user_id)
    .execute(&mut *conn)
    .await
    .context("soft_delete_playbook_tx")?;
    Ok(())
}

/// User-scoped pull deltas. Deliberately does NOT filter `deleted_at IS NULL`: a
/// client that never sees a tombstone can't distinguish "deleted" from "not yet
/// pushed." `>=` (not `>`) re-sends the cursor boundary row, which is harmless
/// because client apply is idempotent.
pub async fn playbooks_since(
    pool: &PgPool,
    user_id: &str,
    cookie: Option<&str>,
) -> Result<Vec<PlaybookDelta>> {
    // A first pull that saw no rows returns `""` as the cursor (unwrap_or_default),
    // and `''::timestamptz` throws. Treat an empty cookie as "no cursor".
    let cookie = cookie.filter(|c| !c.is_empty());
    let sql = format!(
        "SELECT {DELTA_COLS} FROM playbooks \
         WHERE user_id = $1 AND ($2::text IS NULL OR updated_at >= $2::timestamptz) \
         ORDER BY updated_at ASC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(cookie)
        .fetch_all(pool)
        .await
        .context("Failed to read playbook deltas")?;

    let mut out = Vec::with_capacity(rows.len());
    for row in &rows {
        out.push(PlaybookDelta {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            edge_name: row.try_get("edge_name")?,
            entry_rules: row.try_get("entry_rules")?,
            exit_rules: row.try_get("exit_rules")?,
            position_sizing_rules: row.try_get("position_sizing_rules")?,
            additional_rules: row.try_get("additional_rules")?,
            hlc: row.try_get("hlc")?,
            deleted_at: row.try_get("deleted_at")?,
            updated_at: row.try_get("updated_at")?,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{ensure_text, normalize_optional_text};

    #[test]
    fn normalizes_optional_text() {
        assert_eq!(normalize_optional_text(Some("  ".to_string())), None);
        assert_eq!(
            normalize_optional_text(Some(" yes ".to_string())),
            Some("yes".to_string())
        );
    }

    #[test]
    fn empty_name_is_invalid() {
        assert!(
            !ensure_text(&Some("".to_string())),
            "empty string should be invalid"
        );
    }
}
