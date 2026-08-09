use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::tags_table::{self, Tag, TagCategory};

// ---------------------------------------------------------------------------
// Categories
// ---------------------------------------------------------------------------

pub async fn list_categories(user_db: &UserDb, workspace_id: &str) -> Result<Vec<TagCategory>> {
    tags_table::list_categories(user_db.pool(), user_db.user_id(), workspace_id).await
}

pub async fn get_category(user_db: &UserDb, id: &str) -> Result<Option<TagCategory>> {
    tags_table::find_category(user_db.pool(), user_db.user_id(), id).await
}

pub async fn create_category(
    user_db: &UserDb,
    workspace_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<TagCategory> {
    tags_table::create_category(user_db.pool(), user_db.user_id(), workspace_id, name, color).await
}

pub async fn rename_category(user_db: &UserDb, id: &str, name: &str) -> Result<TagCategory> {
    tags_table::rename_category(user_db.pool(), user_db.user_id(), id, name).await
}

pub async fn set_category_color(
    user_db: &UserDb,
    id: &str,
    color: Option<&str>,
) -> Result<TagCategory> {
    tags_table::set_category_color(user_db.pool(), user_db.user_id(), id, color).await
}

pub async fn reorder_categories(user_db: &UserDb, order: &[(String, i64)]) -> Result<()> {
    tags_table::reorder_categories(user_db.pool(), user_db.user_id(), order).await
}

pub async fn delete_category(user_db: &UserDb, id: &str) -> Result<bool> {
    tags_table::delete_category(user_db.pool(), user_db.user_id(), id).await
}

// ---------------------------------------------------------------------------
// Tags
// ---------------------------------------------------------------------------

pub async fn list_tags(
    user_db: &UserDb,
    workspace_id: &str,
    category_id: Option<&str>,
) -> Result<Vec<Tag>> {
    tags_table::list_tags(user_db.pool(), user_db.user_id(), workspace_id, category_id).await
}

pub async fn get_tag(user_db: &UserDb, id: &str) -> Result<Option<Tag>> {
    tags_table::find_tag(user_db.pool(), user_db.user_id(), id).await
}

pub async fn create_tag(
    user_db: &UserDb,
    workspace_id: &str,
    category_id: &str,
    name: &str,
    color: Option<&str>,
) -> Result<Tag> {
    tags_table::create_tag(
        user_db.pool(),
        user_db.user_id(),
        workspace_id,
        category_id,
        name,
        color,
    )
    .await
}

pub async fn rename_tag(user_db: &UserDb, id: &str, name: &str) -> Result<Tag> {
    tags_table::rename_tag(user_db.pool(), user_db.user_id(), id, name).await
}

pub async fn set_tag_color(user_db: &UserDb, id: &str, color: Option<&str>) -> Result<Tag> {
    tags_table::set_tag_color(user_db.pool(), user_db.user_id(), id, color).await
}

pub async fn delete_tag(user_db: &UserDb, id: &str) -> Result<bool> {
    tags_table::delete_tag(user_db.pool(), user_db.user_id(), id).await
}

pub async fn merge_tags(user_db: &UserDb, from_id: &str, into_id: &str) -> Result<()> {
    tags_table::merge_tags(user_db.pool(), user_db.user_id(), from_id, into_id).await
}

// ---------------------------------------------------------------------------
// Trade <-> tag links
// ---------------------------------------------------------------------------

pub async fn set_trade_tags(
    user_db: &UserDb,
    journal_entry_id: &str,
    tag_ids: &[String],
) -> Result<()> {
    tags_table::set_trade_tags(user_db.pool(), user_db.user_id(), journal_entry_id, tag_ids).await
}

pub async fn tags_for_trade(user_db: &UserDb, journal_entry_id: &str) -> Result<Vec<Tag>> {
    tags_table::tags_for_trade(user_db.pool(), user_db.user_id(), journal_entry_id).await
}
