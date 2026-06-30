use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub enum TagRole {
    Mistake,
    Tactic,
    Edge,
}

impl TagRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Mistake => "mistake",
            Self::Tactic => "tactic",
            Self::Edge => "edge",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "mistake" => Some(Self::Mistake),
            "tactic" => Some(Self::Tactic),
            "edge" => Some(Self::Edge),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct TagCategory {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub role: Option<TagRole>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Tag {
    pub id: String,
    pub user_id: String,
    pub category_id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Hydrated tag with its owning category context (for analytics / readers).
#[derive(Clone, Debug, Serialize)]
pub struct TradeTag {
    pub tag: Tag,
    pub category_id: String,
    pub category_name: String,
    pub role: Option<TagRole>,
}

// SELECT column lists. created_at/updated_at are TIMESTAMPTZ in Postgres; the
// struct fields are `String`, so render them to the original RFC3339-ish form.
const CATEGORY_COLS: &str = "id, user_id, name, role, color, sort_order, \
     to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
     to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";
const TAG_COLS: &str = "id, user_id, category_id, name, color, \
     to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
     to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

fn row_to_category(row: &sqlx::postgres::PgRow) -> Result<TagCategory> {
    let role = row
        .try_get::<Option<String>, _>(3)?
        .and_then(|r| TagRole::parse(&r));
    Ok(TagCategory {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        name: row.try_get::<String, _>(2)?,
        role,
        color: row.try_get::<Option<String>, _>(4)?,
        sort_order: row.try_get::<i64, _>(5)?,
        created_at: row.try_get::<String, _>(6)?,
        updated_at: row.try_get::<String, _>(7)?,
    })
}

fn row_to_tag(row: &sqlx::postgres::PgRow) -> Result<Tag> {
    Ok(Tag {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        category_id: row.try_get::<String, _>(2)?,
        name: row.try_get::<String, _>(3)?,
        color: row.try_get::<Option<String>, _>(4)?,
        created_at: row.try_get::<String, _>(5)?,
        updated_at: row.try_get::<String, _>(6)?,
    })
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn is_unique_violation(err: &anyhow::Error) -> bool {
    // Prefer the SQLSTATE code (23505 = unique_violation) when this is a sqlx
    // database error; fall back to a substring match on the message.
    if let Some(sqlx::Error::Database(db_err)) = err.downcast_ref::<sqlx::Error>()
        && db_err.code().as_deref() == Some("23505")
    {
        return true;
    }
    err.to_string().to_lowercase().contains("unique")
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

/// Idempotently seed the three roled default categories for a user.
/// Relies on the partial unique index `idx_tagcat_user_role` so repeated calls
/// are no-ops.
pub async fn ensure_default_categories(pool: &PgPool, user_id: &str) -> Result<()> {
    let defaults = [
        (TagRole::Mistake, "Mistakes", 0i64),
        (TagRole::Tactic, "Tactics", 1),
        (TagRole::Edge, "Edges", 2),
    ];

    let mut tx = pool.begin().await?;
    for (role, name, sort_order) in defaults {
        let now = Utc::now();
        // `ON CONFLICT DO NOTHING` (no target) covers the partial unique index
        // on (user_id, role); INSERT OR IGNORE relied on that index.
        sqlx::query(
            "INSERT INTO tag_categories (id, user_id, name, role, color, sort_order, created_at, updated_at) \
             VALUES ($1, $2, $3, $4, NULL, $5, $6, $6) ON CONFLICT DO NOTHING",
        )
        .bind(new_id())
        .bind(user_id)
        .bind(name)
        .bind(role.as_str())
        .bind(sort_order)
        .bind(now)
        .execute(&mut *tx)
        .await
        .context("Failed to seed default tag category")?;
    }
    tx.commit().await?;

    Ok(())
}

pub async fn list_categories(pool: &PgPool, user_id: &str) -> Result<Vec<TagCategory>> {
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {CATEGORY_COLS} FROM tag_categories WHERE user_id = $1 ORDER BY sort_order, name"
    )))
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to list tag categories")?;

    let mut categories = Vec::new();
    for row in &rows {
        categories.push(row_to_category(row)?);
    }
    Ok(categories)
}

async fn find_category(pool: &PgPool, user_id: &str, id: &str) -> Result<Option<TagCategory>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {CATEGORY_COLS} FROM tag_categories WHERE id = $1 AND user_id = $2"
    )))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to find tag category")?;

    match row {
        Some(row) => Ok(Some(row_to_category(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_category(
    pool: &PgPool,
    user_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<TagCategory> {
    let name = name.trim();
    ensure!(!name.is_empty(), "category name cannot be empty");

    let id = new_id();
    let now = Utc::now();

    let next_sort: i64 = {
        let row = sqlx::query(
            "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM tag_categories WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?;
        match row {
            Some(row) => row.try_get::<i64, _>(0)?,
            None => 0,
        }
    };

    let result = sqlx::query(
        "INSERT INTO tag_categories (id, user_id, name, role, color, sort_order, created_at, updated_at) \
         VALUES ($1, $2, $3, NULL, $4, $5, $6, $6)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(name)
    .bind(color)
    .bind(next_sort)
    .bind(now)
    .execute(pool)
    .await
    .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a category named \"{name}\" already exists");
        }
        return Err(err).context("Failed to insert tag category");
    }

    find_category(pool, user_id, &id)
        .await?
        .context("Category not found after insert")
}

pub async fn rename_category(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    name: &str,
) -> Result<TagCategory> {
    let name = name.trim();
    ensure!(!name.is_empty(), "category name cannot be empty");

    let result = sqlx::query(
        "UPDATE tag_categories SET name = $1, updated_at = $2 WHERE id = $3 AND user_id = $4",
    )
    .bind(name)
    .bind(Utc::now())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a category named \"{name}\" already exists");
        }
        return Err(err).context("Failed to rename tag category");
    }

    find_category(pool, user_id, id)
        .await?
        .context("Category not found after rename")
}

pub async fn set_category_color(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    color: Option<&str>,
) -> Result<TagCategory> {
    sqlx::query(
        "UPDATE tag_categories SET color = $1, updated_at = $2 WHERE id = $3 AND user_id = $4",
    )
    .bind(color)
    .bind(Utc::now())
    .bind(id)
    .bind(user_id)
    .execute(pool)
    .await
    .context("Failed to set tag category color")?;

    find_category(pool, user_id, id)
        .await?
        .context("Category not found after color update")
}

pub async fn reorder_categories(
    pool: &PgPool,
    user_id: &str,
    order: &[(String, i64)],
) -> Result<()> {
    let mut tx = pool.begin().await?;
    let now = Utc::now();
    for (id, sort_order) in order {
        sqlx::query(
            "UPDATE tag_categories SET sort_order = $1, updated_at = $2 WHERE id = $3 AND user_id = $4",
        )
        .bind(*sort_order)
        .bind(now)
        .bind(id.as_str())
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Failed to reorder tag categories")?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_category(pool: &PgPool, user_id: &str, id: &str) -> Result<bool> {
    let category = find_category(pool, user_id, id)
        .await?
        .context("Category not found")?;

    if category.role.is_some() {
        bail!("default categories cannot be deleted");
    }

    let mut tx = pool.begin().await?;

    sqlx::query(
        "DELETE FROM trade_tags WHERE tag_id IN (SELECT id FROM tags WHERE category_id = $1)",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .context("Failed to delete trade_tags for category's tags")?;

    sqlx::query("DELETE FROM tags WHERE category_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete tags for category")?;

    let rows_affected = sqlx::query("DELETE FROM tag_categories WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete tag category")?
        .rows_affected();

    tx.commit().await?;
    Ok(rows_affected > 0)
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

pub async fn list_tags(
    pool: &PgPool,
    user_id: &str,
    category_id: Option<&str>,
) -> Result<Vec<Tag>> {
    let rows =
        match category_id {
            Some(cat) => sqlx::query(sqlx::AssertSqlSafe(format!(
                "SELECT {TAG_COLS} FROM tags WHERE user_id = $1 AND category_id = $2 ORDER BY name"
            )))
            .bind(user_id)
            .bind(cat)
            .fetch_all(pool)
            .await,
            None => {
                sqlx::query(sqlx::AssertSqlSafe(format!(
                    "SELECT {TAG_COLS} FROM tags WHERE user_id = $1 ORDER BY name"
                )))
                .bind(user_id)
                .fetch_all(pool)
                .await
            }
        }
        .context("Failed to list tags")?;

    let mut tags = Vec::new();
    for row in &rows {
        tags.push(row_to_tag(row)?);
    }
    Ok(tags)
}

async fn find_tag(pool: &PgPool, user_id: &str, id: &str) -> Result<Option<Tag>> {
    let row = sqlx::query(sqlx::AssertSqlSafe(format!(
        "SELECT {TAG_COLS} FROM tags WHERE id = $1 AND user_id = $2"
    )))
    .bind(id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .context("Failed to find tag")?;

    match row {
        Some(row) => Ok(Some(row_to_tag(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_tag(
    pool: &PgPool,
    user_id: &str,
    category_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<Tag> {
    let name = name.trim();
    ensure!(!name.is_empty(), "tag name cannot be empty");

    // Validate the category belongs to the user.
    find_category(pool, user_id, category_id)
        .await?
        .context("category not found")?;

    let id = new_id();
    let now = Utc::now();

    let result = sqlx::query(
        "INSERT INTO tags (id, user_id, category_id, name, color, created_at, updated_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $6)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(category_id)
    .bind(name)
    .bind(color)
    .bind(now)
    .execute(pool)
    .await
    .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a tag named \"{name}\" already exists in this category");
        }
        return Err(err).context("Failed to insert tag");
    }

    find_tag(pool, user_id, &id)
        .await?
        .context("Tag not found after insert")
}

pub async fn rename_tag(pool: &PgPool, user_id: &str, id: &str, name: &str) -> Result<Tag> {
    let name = name.trim();
    ensure!(!name.is_empty(), "tag name cannot be empty");

    let result =
        sqlx::query("UPDATE tags SET name = $1, updated_at = $2 WHERE id = $3 AND user_id = $4")
            .bind(name)
            .bind(Utc::now())
            .bind(id)
            .bind(user_id)
            .execute(pool)
            .await
            .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a tag named \"{name}\" already exists in this category");
        }
        return Err(err).context("Failed to rename tag");
    }

    find_tag(pool, user_id, id)
        .await?
        .context("Tag not found after rename")
}

pub async fn set_tag_color(
    pool: &PgPool,
    user_id: &str,
    id: &str,
    color: Option<&str>,
) -> Result<Tag> {
    sqlx::query("UPDATE tags SET color = $1, updated_at = $2 WHERE id = $3 AND user_id = $4")
        .bind(color)
        .bind(Utc::now())
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to set tag color")?;

    find_tag(pool, user_id, id)
        .await?
        .context("Tag not found after color update")
}

pub async fn delete_tag(pool: &PgPool, user_id: &str, id: &str) -> Result<bool> {
    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM trade_tags WHERE tag_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete trade_tags for tag")?;

    let rows_affected = sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete tag")?
        .rows_affected();

    tx.commit().await?;
    Ok(rows_affected > 0)
}

/// Merge `from_id` into `into_id`. Both tags must belong to the user and be in
/// the same category. Repoints all `trade_tags` links to `into_id` (dedup via
/// ON CONFLICT DO NOTHING), removes the old links, then deletes the `from` tag.
pub async fn merge_tags(pool: &PgPool, user_id: &str, from_id: &str, into_id: &str) -> Result<()> {
    ensure!(from_id != into_id, "cannot merge a tag into itself");

    let from = find_tag(pool, user_id, from_id)
        .await?
        .context("source tag not found")?;
    let into = find_tag(pool, user_id, into_id)
        .await?
        .context("target tag not found")?;

    ensure!(
        from.category_id == into.category_id,
        "tags must be in the same category to merge"
    );

    let mut tx = pool.begin().await?;

    sqlx::query(
        "INSERT INTO trade_tags (journal_entry_id, tag_id) \
         SELECT journal_entry_id, $1 FROM trade_tags WHERE tag_id = $2 \
         ON CONFLICT (journal_entry_id, tag_id) DO NOTHING",
    )
    .bind(into_id)
    .bind(from_id)
    .execute(&mut *tx)
    .await
    .context("Failed to repoint trade_tags during merge")?;

    sqlx::query("DELETE FROM trade_tags WHERE tag_id = $1")
        .bind(from_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear old trade_tags during merge")?;

    sqlx::query("DELETE FROM tags WHERE id = $1 AND user_id = $2")
        .bind(from_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await
        .context("Failed to delete source tag during merge")?;

    tx.commit().await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Trade <-> tag links
// ---------------------------------------------------------------------------

/// Replace a trade's tag links with exactly the given set. Validates that each
/// tag belongs to the user.
pub async fn set_trade_tags(
    pool: &PgPool,
    user_id: &str,
    journal_entry_id: &str,
    tag_ids: &[String],
) -> Result<()> {
    // Validate ownership of every tag first.
    for tag_id in tag_ids {
        find_tag(pool, user_id, tag_id)
            .await?
            .with_context(|| format!("tag {tag_id} not found"))?;
    }

    let mut tx = pool.begin().await?;

    sqlx::query("DELETE FROM trade_tags WHERE journal_entry_id = $1")
        .bind(journal_entry_id)
        .execute(&mut *tx)
        .await
        .context("Failed to clear existing trade_tags")?;

    for tag_id in tag_ids {
        sqlx::query(
            "INSERT INTO trade_tags (journal_entry_id, tag_id) VALUES ($1, $2) \
             ON CONFLICT (journal_entry_id, tag_id) DO NOTHING",
        )
        .bind(journal_entry_id)
        .bind(tag_id.as_str())
        .execute(&mut *tx)
        .await
        .context("Failed to insert trade_tag")?;
    }

    tx.commit().await?;
    Ok(())
}

/// All tags attached to a single trade. Scoped to `user_id` as belt-and-suspenders
/// (a trade's tags all belong to its user).
pub async fn tags_for_trade(
    pool: &PgPool,
    user_id: &str,
    journal_entry_id: &str,
) -> Result<Vec<Tag>> {
    let rows = sqlx::query(
        "SELECT t.id, t.user_id, t.category_id, t.name, t.color, \
                to_char(t.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
                to_char(t.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at \
         FROM tags t JOIN trade_tags tt ON tt.tag_id = t.id \
         WHERE tt.journal_entry_id = $1 AND t.user_id = $2 ORDER BY t.name",
    )
    .bind(journal_entry_id)
    .bind(user_id)
    .fetch_all(pool)
    .await
    .context("Failed to load tags for trade")?;

    let mut tags = Vec::new();
    for row in &rows {
        tags.push(row_to_tag(row)?);
    }
    Ok(tags)
}

/// Batch hydration: maps each journal_entry_id to its hydrated tags (with
/// category context). Trades with no tags are absent from the map.
pub async fn tags_for_trades(
    pool: &PgPool,
    journal_entry_ids: &[String],
) -> Result<HashMap<String, Vec<TradeTag>>> {
    let mut map: HashMap<String, Vec<TradeTag>> = HashMap::new();
    if journal_entry_ids.is_empty() {
        return Ok(map);
    }

    let placeholders = journal_entry_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("${}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT tt.journal_entry_id, \
                t.id, t.user_id, t.category_id, t.name, t.color, \
                to_char(t.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
                to_char(t.updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at, \
                c.name, c.role \
         FROM trade_tags tt \
         JOIN tags t ON t.id = tt.tag_id \
         JOIN tag_categories c ON c.id = t.category_id \
         WHERE tt.journal_entry_id IN ({placeholders}) \
         ORDER BY c.sort_order, c.name, t.name"
    );

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for id in journal_entry_ids {
        query = query.bind(id);
    }

    let rows = query
        .fetch_all(pool)
        .await
        .context("Failed to batch-load tags for trades")?;

    for row in &rows {
        let journal_entry_id = row.try_get::<String, _>(0)?;
        let tag = Tag {
            id: row.try_get::<String, _>(1)?,
            user_id: row.try_get::<String, _>(2)?,
            category_id: row.try_get::<String, _>(3)?,
            name: row.try_get::<String, _>(4)?,
            color: row.try_get::<Option<String>, _>(5)?,
            created_at: row.try_get::<String, _>(6)?,
            updated_at: row.try_get::<String, _>(7)?,
        };
        let category_id = tag.category_id.clone();
        let category_name = row.try_get::<String, _>(8)?;
        let role = row
            .try_get::<Option<String>, _>(9)?
            .and_then(|r| TagRole::parse(&r));

        map.entry(journal_entry_id).or_default().push(TradeTag {
            tag,
            category_id,
            category_name,
            role,
        });
    }

    Ok(map)
}
