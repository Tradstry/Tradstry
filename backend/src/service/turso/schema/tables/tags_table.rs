use anyhow::{Context, Result, bail, ensure};
use chrono::Utc;
use libsql::Connection;
use serde::Serialize;
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

const CATEGORY_COLS: &str = "id, user_id, name, role, color, sort_order, created_at, updated_at";
const TAG_COLS: &str = "id, user_id, category_id, name, color, created_at, updated_at";

// ---------------------------------------------------------------------------
// Row mappers
// ---------------------------------------------------------------------------

fn opt_text(value: Option<libsql::Value>) -> Option<String> {
    match value {
        Some(libsql::Value::Text(text)) => Some(text),
        _ => None,
    }
}

fn row_to_category(row: &libsql::Row) -> Result<TagCategory> {
    let role = opt_text(row.get::<libsql::Value>(3).ok()).and_then(|r| TagRole::parse(&r));
    Ok(TagCategory {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        name: row.get::<String>(2)?,
        role,
        color: opt_text(row.get::<libsql::Value>(4).ok()),
        sort_order: row.get::<i64>(5)?,
        created_at: row.get::<String>(6)?,
        updated_at: row.get::<String>(7)?,
    })
}

fn row_to_tag(row: &libsql::Row) -> Result<Tag> {
    Ok(Tag {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        category_id: row.get::<String>(2)?,
        name: row.get::<String>(3)?,
        color: opt_text(row.get::<libsql::Value>(4).ok()),
        created_at: row.get::<String>(5)?,
        updated_at: row.get::<String>(6)?,
    })
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn new_id() -> String {
    Uuid::new_v4().to_string()
}

fn is_unique_violation(err: &anyhow::Error) -> bool {
    err.to_string().to_lowercase().contains("unique")
}

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

/// Idempotently seed the three roled default categories for a user.
/// Relies on the partial unique index `idx_tagcat_user_role` so repeated calls
/// are no-ops.
pub async fn ensure_default_categories(conn: &Connection, user_id: &str) -> Result<()> {
    let defaults = [
        (TagRole::Mistake, "Mistakes", 0i64),
        (TagRole::Tactic, "Tactics", 1),
        (TagRole::Edge, "Edges", 2),
    ];

    for (role, name, sort_order) in defaults {
        let ts = now();
        conn.execute(
            "INSERT OR IGNORE INTO tag_categories (id, user_id, name, role, color, sort_order, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?6, ?6)",
            libsql::params![
                new_id(),
                user_id,
                name,
                role.as_str(),
                sort_order,
                ts.as_str(),
            ],
        )
        .await
        .context("Failed to seed default tag category")?;
    }

    Ok(())
}

pub async fn list_categories(conn: &Connection, user_id: &str) -> Result<Vec<TagCategory>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {CATEGORY_COLS} FROM tag_categories WHERE user_id = ?1 ORDER BY sort_order, name"
            ),
            libsql::params![user_id],
        )
        .await
        .context("Failed to list tag categories")?;

    let mut categories = Vec::new();
    while let Some(row) = rows.next().await? {
        categories.push(row_to_category(&row)?);
    }
    Ok(categories)
}

async fn find_category(conn: &Connection, user_id: &str, id: &str) -> Result<Option<TagCategory>> {
    let mut rows = conn
        .query(
            &format!("SELECT {CATEGORY_COLS} FROM tag_categories WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find tag category")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_category(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_category(
    conn: &Connection,
    user_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<TagCategory> {
    let name = name.trim();
    ensure!(!name.is_empty(), "category name cannot be empty");

    let id = new_id();
    let ts = now();

    let next_sort: i64 = {
        let mut rows = conn
            .query(
                "SELECT COALESCE(MAX(sort_order) + 1, 0) FROM tag_categories WHERE user_id = ?1",
                libsql::params![user_id],
            )
            .await?;
        match rows.next().await? {
            Some(row) => row.get::<i64>(0)?,
            None => 0,
        }
    };

    let result = conn
        .execute(
            "INSERT INTO tag_categories (id, user_id, name, role, color, sort_order, created_at, updated_at) \
             VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?6)",
            libsql::params![
                id.as_str(),
                user_id,
                name,
                color,
                next_sort,
                ts.as_str(),
            ],
        )
        .await
        .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a category named \"{name}\" already exists");
        }
        return Err(err).context("Failed to insert tag category");
    }

    find_category(conn, user_id, &id)
        .await?
        .context("Category not found after insert")
}

pub async fn rename_category(
    conn: &Connection,
    user_id: &str,
    id: &str,
    name: &str,
) -> Result<TagCategory> {
    let name = name.trim();
    ensure!(!name.is_empty(), "category name cannot be empty");

    let result = conn
        .execute(
            "UPDATE tag_categories SET name = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            libsql::params![name, now().as_str(), id, user_id],
        )
        .await
        .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a category named \"{name}\" already exists");
        }
        return Err(err).context("Failed to rename tag category");
    }

    find_category(conn, user_id, id)
        .await?
        .context("Category not found after rename")
}

pub async fn set_category_color(
    conn: &Connection,
    user_id: &str,
    id: &str,
    color: Option<&str>,
) -> Result<TagCategory> {
    conn.execute(
        "UPDATE tag_categories SET color = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
        libsql::params![color, now().as_str(), id, user_id],
    )
    .await
    .context("Failed to set tag category color")?;

    find_category(conn, user_id, id)
        .await?
        .context("Category not found after color update")
}

pub async fn reorder_categories(
    conn: &Connection,
    user_id: &str,
    order: &[(String, i64)],
) -> Result<()> {
    let tx = conn.transaction().await?;
    let ts = now();
    for (id, sort_order) in order {
        tx.execute(
            "UPDATE tag_categories SET sort_order = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            libsql::params![*sort_order, ts.as_str(), id.as_str(), user_id],
        )
        .await
        .context("Failed to reorder tag categories")?;
    }
    tx.commit().await?;
    Ok(())
}

pub async fn delete_category(conn: &Connection, user_id: &str, id: &str) -> Result<bool> {
    let category = find_category(conn, user_id, id)
        .await?
        .context("Category not found")?;

    if category.role.is_some() {
        bail!("default categories cannot be deleted");
    }

    let tx = conn.transaction().await?;

    tx.execute(
        "DELETE FROM trade_tags WHERE tag_id IN (SELECT id FROM tags WHERE category_id = ?1)",
        libsql::params![id],
    )
    .await
    .context("Failed to delete trade_tags for category's tags")?;

    tx.execute(
        "DELETE FROM tags WHERE category_id = ?1",
        libsql::params![id],
    )
    .await
    .context("Failed to delete tags for category")?;

    let rows_affected = tx
        .execute(
            "DELETE FROM tag_categories WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete tag category")?;

    tx.commit().await?;
    Ok(rows_affected > 0)
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

pub async fn list_tags(
    conn: &Connection,
    user_id: &str,
    category_id: Option<&str>,
) -> Result<Vec<Tag>> {
    let mut rows = match category_id {
        Some(cat) => {
            conn.query(
                &format!(
                    "SELECT {TAG_COLS} FROM tags WHERE user_id = ?1 AND category_id = ?2 ORDER BY name"
                ),
                libsql::params![user_id, cat],
            )
            .await
        }
        None => {
            conn.query(
                &format!("SELECT {TAG_COLS} FROM tags WHERE user_id = ?1 ORDER BY name"),
                libsql::params![user_id],
            )
            .await
        }
    }
    .context("Failed to list tags")?;

    let mut tags = Vec::new();
    while let Some(row) = rows.next().await? {
        tags.push(row_to_tag(&row)?);
    }
    Ok(tags)
}

async fn find_tag(conn: &Connection, user_id: &str, id: &str) -> Result<Option<Tag>> {
    let mut rows = conn
        .query(
            &format!("SELECT {TAG_COLS} FROM tags WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find tag")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_tag(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_tag(
    conn: &Connection,
    user_id: &str,
    category_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<Tag> {
    let name = name.trim();
    ensure!(!name.is_empty(), "tag name cannot be empty");

    // Validate the category belongs to the user.
    find_category(conn, user_id, category_id)
        .await?
        .context("category not found")?;

    let id = new_id();
    let ts = now();

    let result = conn
        .execute(
            "INSERT INTO tags (id, user_id, category_id, name, color, created_at, updated_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)",
            libsql::params![id.as_str(), user_id, category_id, name, color, ts.as_str(),],
        )
        .await
        .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a tag named \"{name}\" already exists in this category");
        }
        return Err(err).context("Failed to insert tag");
    }

    find_tag(conn, user_id, &id)
        .await?
        .context("Tag not found after insert")
}

pub async fn rename_tag(conn: &Connection, user_id: &str, id: &str, name: &str) -> Result<Tag> {
    let name = name.trim();
    ensure!(!name.is_empty(), "tag name cannot be empty");

    let result = conn
        .execute(
            "UPDATE tags SET name = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
            libsql::params![name, now().as_str(), id, user_id],
        )
        .await
        .map_err(anyhow::Error::from);

    if let Err(err) = result {
        if is_unique_violation(&err) {
            bail!("a tag named \"{name}\" already exists in this category");
        }
        return Err(err).context("Failed to rename tag");
    }

    find_tag(conn, user_id, id)
        .await?
        .context("Tag not found after rename")
}

pub async fn set_tag_color(
    conn: &Connection,
    user_id: &str,
    id: &str,
    color: Option<&str>,
) -> Result<Tag> {
    conn.execute(
        "UPDATE tags SET color = ?1, updated_at = ?2 WHERE id = ?3 AND user_id = ?4",
        libsql::params![color, now().as_str(), id, user_id],
    )
    .await
    .context("Failed to set tag color")?;

    find_tag(conn, user_id, id)
        .await?
        .context("Tag not found after color update")
}

pub async fn delete_tag(conn: &Connection, user_id: &str, id: &str) -> Result<bool> {
    let tx = conn.transaction().await?;

    tx.execute(
        "DELETE FROM trade_tags WHERE tag_id = ?1",
        libsql::params![id],
    )
    .await
    .context("Failed to delete trade_tags for tag")?;

    let rows_affected = tx
        .execute(
            "DELETE FROM tags WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete tag")?;

    tx.commit().await?;
    Ok(rows_affected > 0)
}

/// Merge `from_id` into `into_id`. Both tags must belong to the user and be in
/// the same category. Repoints all `trade_tags` links to `into_id` (dedup via
/// INSERT OR IGNORE), removes the old links, then deletes the `from` tag.
pub async fn merge_tags(
    conn: &Connection,
    user_id: &str,
    from_id: &str,
    into_id: &str,
) -> Result<()> {
    ensure!(from_id != into_id, "cannot merge a tag into itself");

    let from = find_tag(conn, user_id, from_id)
        .await?
        .context("source tag not found")?;
    let into = find_tag(conn, user_id, into_id)
        .await?
        .context("target tag not found")?;

    ensure!(
        from.category_id == into.category_id,
        "tags must be in the same category to merge"
    );

    let tx = conn.transaction().await?;

    tx.execute(
        "INSERT OR IGNORE INTO trade_tags (journal_entry_id, tag_id) \
         SELECT journal_entry_id, ?1 FROM trade_tags WHERE tag_id = ?2",
        libsql::params![into_id, from_id],
    )
    .await
    .context("Failed to repoint trade_tags during merge")?;

    tx.execute(
        "DELETE FROM trade_tags WHERE tag_id = ?1",
        libsql::params![from_id],
    )
    .await
    .context("Failed to clear old trade_tags during merge")?;

    tx.execute(
        "DELETE FROM tags WHERE id = ?1 AND user_id = ?2",
        libsql::params![from_id, user_id],
    )
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
    conn: &Connection,
    user_id: &str,
    journal_entry_id: &str,
    tag_ids: &[String],
) -> Result<()> {
    // Validate ownership of every tag first.
    for tag_id in tag_ids {
        find_tag(conn, user_id, tag_id)
            .await?
            .with_context(|| format!("tag {tag_id} not found"))?;
    }

    let tx = conn.transaction().await?;

    tx.execute(
        "DELETE FROM trade_tags WHERE journal_entry_id = ?1",
        libsql::params![journal_entry_id],
    )
    .await
    .context("Failed to clear existing trade_tags")?;

    for tag_id in tag_ids {
        tx.execute(
            "INSERT OR IGNORE INTO trade_tags (journal_entry_id, tag_id) VALUES (?1, ?2)",
            libsql::params![journal_entry_id, tag_id.as_str()],
        )
        .await
        .context("Failed to insert trade_tag")?;
    }

    tx.commit().await?;
    Ok(())
}

/// All tags attached to a single trade. Scoped to `user_id` as belt-and-suspenders
/// (a trade's tags all belong to its user).
pub async fn tags_for_trade(
    conn: &Connection,
    user_id: &str,
    journal_entry_id: &str,
) -> Result<Vec<Tag>> {
    let mut rows = conn
        .query(
            "SELECT t.id, t.user_id, t.category_id, t.name, t.color, t.created_at, t.updated_at \
             FROM tags t JOIN trade_tags tt ON tt.tag_id = t.id \
             WHERE tt.journal_entry_id = ?1 AND t.user_id = ?2 ORDER BY t.name",
            libsql::params![journal_entry_id, user_id],
        )
        .await
        .context("Failed to load tags for trade")?;

    let mut tags = Vec::new();
    while let Some(row) = rows.next().await? {
        tags.push(row_to_tag(&row)?);
    }
    Ok(tags)
}

/// Batch hydration: maps each journal_entry_id to its hydrated tags (with
/// category context). Trades with no tags are absent from the map.
pub async fn tags_for_trades(
    conn: &Connection,
    journal_entry_ids: &[String],
) -> Result<HashMap<String, Vec<TradeTag>>> {
    let mut map: HashMap<String, Vec<TradeTag>> = HashMap::new();
    if journal_entry_ids.is_empty() {
        return Ok(map);
    }

    let placeholders = journal_entry_ids
        .iter()
        .map(|_| "?")
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!(
        "SELECT tt.journal_entry_id, \
                t.id, t.user_id, t.category_id, t.name, t.color, t.created_at, t.updated_at, \
                c.name, c.role \
         FROM trade_tags tt \
         JOIN tags t ON t.id = tt.tag_id \
         JOIN tag_categories c ON c.id = t.category_id \
         WHERE tt.journal_entry_id IN ({placeholders}) \
         ORDER BY c.sort_order, c.name, t.name"
    );

    let params: Vec<libsql::Value> = journal_entry_ids
        .iter()
        .map(|id| libsql::Value::Text(id.clone()))
        .collect();

    let mut rows = conn
        .query(&sql, params)
        .await
        .context("Failed to batch-load tags for trades")?;

    while let Some(row) = rows.next().await? {
        let journal_entry_id = row.get::<String>(0)?;
        let tag = Tag {
            id: row.get::<String>(1)?,
            user_id: row.get::<String>(2)?,
            category_id: row.get::<String>(3)?,
            name: row.get::<String>(4)?,
            color: opt_text(row.get::<libsql::Value>(5).ok()),
            created_at: row.get::<String>(6)?,
            updated_at: row.get::<String>(7)?,
        };
        let category_id = tag.category_id.clone();
        let category_name = row.get::<String>(8)?;
        let role = opt_text(row.get::<libsql::Value>(9).ok()).and_then(|r| TagRole::parse(&r));

        map.entry(journal_entry_id).or_default().push(TradeTag {
            tag,
            category_id,
            category_name,
            role,
        });
    }

    Ok(map)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::turso::schema::migrate;
    use libsql::Builder;

    const USER: &str = "user-1";

    /// In-memory libsql connection with the full schema applied and FK
    /// enforcement enabled (libsql requires PRAGMA foreign_keys=ON per
    /// connection for ON DELETE CASCADE to fire).
    async fn setup() -> Connection {
        let db = Builder::new_local(":memory:").build().await.unwrap();
        let conn = db.connect().unwrap();
        conn.execute("PRAGMA foreign_keys = ON", libsql::params![])
            .await
            .unwrap();
        migrate(&conn).await.unwrap();

        // Seed the prerequisite user + account so journal_entries FKs hold.
        conn.execute(
            "INSERT INTO users (id, clerk_uuid) VALUES (?1, ?2)",
            libsql::params![USER, "clerk-1"],
        )
        .await
        .unwrap();
        conn.execute(
            "INSERT INTO accounts (id, user_id, name) VALUES (?1, ?2, ?3)",
            libsql::params!["acct-1", USER, "Main"],
        )
        .await
        .unwrap();
        conn
    }

    async fn insert_trade(conn: &Connection, id: &str) {
        conn.execute(
            "INSERT INTO journal_entries (id, user_id, account_id, open_date, close_date, \
             entry_price, exit_price, position_size, symbol, symbol_name, status, total_pl, \
             net_roi, duration, stop_loss, risk_reward, trade_type, mistakes, entry_tactics, \
             edges_spotted) VALUES (?1, ?2, 'acct-1', '2026-01-01', '2026-01-02', 100.0, 110.0, \
             10.0, 'AAA', 'AAA Inc', 'profit', 100.0, 10.0, 86400, 95.0, 2.0, 'long', '', '', '')",
            libsql::params![id, USER],
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn ensure_default_categories_is_idempotent() {
        let conn = setup().await;
        ensure_default_categories(&conn, USER).await.unwrap();
        ensure_default_categories(&conn, USER).await.unwrap();

        let cats = list_categories(&conn, USER).await.unwrap();
        let roled: Vec<_> = cats.iter().filter(|c| c.role.is_some()).collect();
        assert_eq!(roled.len(), 3, "expected exactly 3 roled categories");

        let roles: Vec<_> = roled.iter().filter_map(|c| c.role.clone()).collect();
        assert!(roles.contains(&TagRole::Mistake));
        assert!(roles.contains(&TagRole::Tactic));
        assert!(roles.contains(&TagRole::Edge));
    }

    #[tokio::test]
    async fn create_tag_rejects_duplicate_name_case_insensitive() {
        let conn = setup().await;
        let cat = create_category(&conn, USER, "Setups", None).await.unwrap();
        create_tag(&conn, USER, &cat.id, "Breakout", None)
            .await
            .unwrap();

        let err = create_tag(&conn, USER, &cat.id, "breakout", None)
            .await
            .unwrap_err();
        assert!(
            err.to_string().to_lowercase().contains("already exists"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn merge_tags_repoints_dedupes_and_deletes_source() {
        let conn = setup().await;
        let cat = create_category(&conn, USER, "Setups", None).await.unwrap();
        let from = create_tag(&conn, USER, &cat.id, "Breakout", None)
            .await
            .unwrap();
        let into = create_tag(&conn, USER, &cat.id, "Breakouts", None)
            .await
            .unwrap();

        insert_trade(&conn, "trade-1").await; // tagged only `from`
        insert_trade(&conn, "trade-2").await; // tagged both (forces dedupe)
        set_trade_tags(&conn, USER, "trade-1", std::slice::from_ref(&from.id))
            .await
            .unwrap();
        set_trade_tags(&conn, USER, "trade-2", &[from.id.clone(), into.id.clone()])
            .await
            .unwrap();

        merge_tags(&conn, USER, &from.id, &into.id).await.unwrap();

        // Source tag gone.
        assert!(find_tag(&conn, USER, &from.id).await.unwrap().is_none());

        // trade-1 now tagged `into`, trade-2 has exactly one (deduped) `into`.
        let t1 = tags_for_trade(&conn, USER, "trade-1").await.unwrap();
        assert_eq!(t1.len(), 1);
        assert_eq!(t1[0].id, into.id);

        let t2 = tags_for_trade(&conn, USER, "trade-2").await.unwrap();
        assert_eq!(t2.len(), 1);
        assert_eq!(t2[0].id, into.id);
    }

    #[tokio::test]
    async fn merge_tags_rejects_cross_category() {
        let conn = setup().await;
        let cat_a = create_category(&conn, USER, "A", None).await.unwrap();
        let cat_b = create_category(&conn, USER, "B", None).await.unwrap();
        let from = create_tag(&conn, USER, &cat_a.id, "x", None).await.unwrap();
        let into = create_tag(&conn, USER, &cat_b.id, "y", None).await.unwrap();

        let err = merge_tags(&conn, USER, &from.id, &into.id)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("same category"), "got: {err}");
    }

    #[tokio::test]
    async fn delete_tag_cascades_links() {
        let conn = setup().await;
        let cat = create_category(&conn, USER, "Setups", None).await.unwrap();
        let tag = create_tag(&conn, USER, &cat.id, "Breakout", None)
            .await
            .unwrap();
        insert_trade(&conn, "trade-1").await;
        set_trade_tags(&conn, USER, "trade-1", std::slice::from_ref(&tag.id))
            .await
            .unwrap();

        assert_eq!(
            tags_for_trade(&conn, USER, "trade-1").await.unwrap().len(),
            1
        );

        delete_tag(&conn, USER, &tag.id).await.unwrap();
        assert_eq!(
            tags_for_trade(&conn, USER, "trade-1").await.unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn delete_category_cascades_and_rejects_roled() {
        let conn = setup().await;
        ensure_default_categories(&conn, USER).await.unwrap();

        // Roled (default) category deletion is rejected.
        let roled = list_categories(&conn, USER)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.role.is_some())
            .unwrap();
        let err = delete_category(&conn, USER, &roled.id).await.unwrap_err();
        assert!(err.to_string().contains("cannot be deleted"), "got: {err}");

        // Custom category cascades to tags + trade_tags.
        let cat = create_category(&conn, USER, "Custom", None).await.unwrap();
        let tag = create_tag(&conn, USER, &cat.id, "Breakout", None)
            .await
            .unwrap();
        insert_trade(&conn, "trade-1").await;
        set_trade_tags(&conn, USER, "trade-1", std::slice::from_ref(&tag.id))
            .await
            .unwrap();

        delete_category(&conn, USER, &cat.id).await.unwrap();

        assert!(
            list_tags(&conn, USER, Some(&cat.id))
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            tags_for_trade(&conn, USER, "trade-1").await.unwrap().len(),
            0
        );
    }

    #[tokio::test]
    async fn set_trade_tags_replaces_exactly() {
        let conn = setup().await;
        let cat = create_category(&conn, USER, "Setups", None).await.unwrap();
        let a = create_tag(&conn, USER, &cat.id, "A", None).await.unwrap();
        let b = create_tag(&conn, USER, &cat.id, "B", None).await.unwrap();
        let c = create_tag(&conn, USER, &cat.id, "C", None).await.unwrap();
        insert_trade(&conn, "trade-1").await;

        set_trade_tags(&conn, USER, "trade-1", &[a.id.clone(), b.id.clone()])
            .await
            .unwrap();
        let mut ids: Vec<_> = tags_for_trade(&conn, USER, "trade-1")
            .await
            .unwrap()
            .into_iter()
            .map(|t| t.id)
            .collect();
        ids.sort();
        let mut expected = vec![a.id.clone(), b.id.clone()];
        expected.sort();
        assert_eq!(ids, expected);

        // Replace with a different exact set.
        set_trade_tags(&conn, USER, "trade-1", std::slice::from_ref(&c.id))
            .await
            .unwrap();
        let after = tags_for_trade(&conn, USER, "trade-1").await.unwrap();
        assert_eq!(after.len(), 1);
        assert_eq!(after[0].id, c.id);
    }

    #[tokio::test]
    async fn tags_for_trades_batch_hydrates_with_category_context() {
        let conn = setup().await;
        ensure_default_categories(&conn, USER).await.unwrap();
        let tactic = list_categories(&conn, USER)
            .await
            .unwrap()
            .into_iter()
            .find(|c| c.role == Some(TagRole::Tactic))
            .unwrap();
        let tag = create_tag(&conn, USER, &tactic.id, "Pullback", None)
            .await
            .unwrap();
        insert_trade(&conn, "trade-1").await;
        set_trade_tags(&conn, USER, "trade-1", std::slice::from_ref(&tag.id))
            .await
            .unwrap();

        let map = tags_for_trades(&conn, &["trade-1".to_string()])
            .await
            .unwrap();
        let hydrated = map.get("trade-1").unwrap();
        assert_eq!(hydrated.len(), 1);
        assert_eq!(hydrated[0].role, Some(TagRole::Tactic));
        assert_eq!(hydrated[0].category_id, tactic.id);
    }
}
