use async_graphql::{Context, Enum, InputObject, Object, Result, SimpleObject};
use clerk_rs::validators::authorizer::ClerkJwt;
use std::sync::Arc;

use crate::service::read_service::tags as tags_service;
use crate::service::read_service::users::ensure_user;
use crate::service::turso::TursoClient;
use crate::service::turso::schema::tables::tags_table::{Tag, TagCategory, TagRole};

async fn get_user_db(ctx: &Context<'_>) -> Result<crate::service::turso::client::UserDb> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let turso = ctx.data::<Arc<TursoClient>>()?;
    let conn = turso.get_connection()?;

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let user = ensure_user(&conn, &jwt.sub, full_name, email).await?;

    Ok(turso.get_user_db(&user.id).await?)
}

// ---------------------------------------------------------------------------
// GraphQL types
// ---------------------------------------------------------------------------

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum TagRoleGql {
    Mistake,
    Tactic,
    Edge,
}

impl From<TagRole> for TagRoleGql {
    fn from(role: TagRole) -> Self {
        match role {
            TagRole::Mistake => Self::Mistake,
            TagRole::Tactic => Self::Tactic,
            TagRole::Edge => Self::Edge,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct TagCategoryGql {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub role: Option<TagRoleGql>,
    pub color: Option<String>,
    pub sort_order: i64,
    pub created_at: String,
    pub updated_at: String,
}

impl From<TagCategory> for TagCategoryGql {
    fn from(c: TagCategory) -> Self {
        Self {
            id: c.id,
            user_id: c.user_id,
            name: c.name,
            role: c.role.map(TagRoleGql::from),
            color: c.color,
            sort_order: c.sort_order,
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct TagGql {
    pub id: String,
    pub user_id: String,
    pub category_id: String,
    pub name: String,
    pub color: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Tag> for TagGql {
    fn from(t: Tag) -> Self {
        Self {
            id: t.id,
            user_id: t.user_id,
            category_id: t.category_id,
            name: t.name,
            color: t.color,
            created_at: t.created_at,
            updated_at: t.updated_at,
        }
    }
}

#[derive(Debug, InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct ReorderTagCategoryInput {
    pub id: String,
    pub sort_order: i64,
}

// ---------------------------------------------------------------------------
// Query
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TagQuery;

#[Object]
impl TagQuery {
    async fn tag_categories(&self, ctx: &Context<'_>) -> Result<Vec<TagCategoryGql>> {
        let user_db = get_user_db(ctx).await?;
        // Lazily backfill defaults here (the tags UI path), so the universal
        // auth path doesn't pay 3 remote writes on every request.
        crate::service::turso::schema::tables::tags_table::ensure_default_categories(
            user_db.conn(),
            user_db.user_id(),
        )
        .await?;
        Ok(tags_service::list_categories(&user_db)
            .await?
            .into_iter()
            .map(TagCategoryGql::from)
            .collect())
    }

    async fn tags(&self, ctx: &Context<'_>, category_id: Option<String>) -> Result<Vec<TagGql>> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::list_tags(&user_db, category_id.as_deref())
            .await?
            .into_iter()
            .map(TagGql::from)
            .collect())
    }
}

// ---------------------------------------------------------------------------
// Mutation
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TagMutation;

#[Object]
impl TagMutation {
    async fn create_tag_category(
        &self,
        ctx: &Context<'_>,
        name: String,
        color: Option<String>,
    ) -> Result<TagCategoryGql> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            tags_service::create_category(&user_db, &name, color.as_deref())
                .await?
                .into(),
        )
    }

    async fn rename_tag_category(
        &self,
        ctx: &Context<'_>,
        id: String,
        name: String,
    ) -> Result<TagCategoryGql> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::rename_category(&user_db, &id, &name)
            .await?
            .into())
    }

    async fn set_tag_category_color(
        &self,
        ctx: &Context<'_>,
        id: String,
        color: Option<String>,
    ) -> Result<TagCategoryGql> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            tags_service::set_category_color(&user_db, &id, color.as_deref())
                .await?
                .into(),
        )
    }

    async fn reorder_tag_categories(
        &self,
        ctx: &Context<'_>,
        order: Vec<ReorderTagCategoryInput>,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        let order: Vec<(String, i64)> = order.into_iter().map(|o| (o.id, o.sort_order)).collect();
        tags_service::reorder_categories(&user_db, &order).await?;
        Ok(true)
    }

    async fn delete_tag_category(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::delete_category(&user_db, &id).await?)
    }

    async fn create_tag(
        &self,
        ctx: &Context<'_>,
        category_id: String,
        name: String,
        color: Option<String>,
    ) -> Result<TagGql> {
        let user_db = get_user_db(ctx).await?;
        Ok(
            tags_service::create_tag(&user_db, &category_id, &name, color.as_deref())
                .await?
                .into(),
        )
    }

    async fn rename_tag(&self, ctx: &Context<'_>, id: String, name: String) -> Result<TagGql> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::rename_tag(&user_db, &id, &name).await?.into())
    }

    async fn set_tag_color(
        &self,
        ctx: &Context<'_>,
        id: String,
        color: Option<String>,
    ) -> Result<TagGql> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::set_tag_color(&user_db, &id, color.as_deref())
            .await?
            .into())
    }

    async fn delete_tag(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        Ok(tags_service::delete_tag(&user_db, &id).await?)
    }

    async fn merge_tags(
        &self,
        ctx: &Context<'_>,
        from_id: String,
        into_id: String,
    ) -> Result<bool> {
        let user_db = get_user_db(ctx).await?;
        tags_service::merge_tags(&user_db, &from_id, &into_id).await?;
        Ok(true)
    }
}
