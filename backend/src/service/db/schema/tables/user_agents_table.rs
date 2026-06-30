use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct UserAgent {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub name: String,
    pub goal: String,
    pub steps_json: String,
    pub output_style: String,
    pub config_json: String,
    pub created_at: String,
    pub updated_at: String,
}

const SELECT_COLS: &str = "id, user_id, account_id, name, goal, steps_json, output_style, config_json, \
    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at, \
    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at";

fn row_to_user_agent(row: &sqlx::postgres::PgRow) -> Result<UserAgent> {
    Ok(UserAgent {
        id: row.try_get::<String, _>(0)?,
        user_id: row.try_get::<String, _>(1)?,
        account_id: row.try_get::<String, _>(2)?,
        name: row.try_get::<String, _>(3)?,
        goal: row.try_get::<String, _>(4)?,
        steps_json: row.try_get::<String, _>(5)?,
        output_style: row.try_get::<String, _>(6)?,
        config_json: row.try_get::<String, _>(7)?,
        created_at: row.try_get::<String, _>(8)?,
        updated_at: row.try_get::<String, _>(9)?,
    })
}

pub async fn list_user_agents(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<UserAgent>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM user_agents WHERE user_id = $1 AND account_id = $2 ORDER BY created_at DESC"
    );
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(user_id)
        .bind(account_id)
        .fetch_all(pool)
        .await
        .context("Failed to list user agents")?;

    let mut agents = Vec::new();
    for row in &rows {
        agents.push(row_to_user_agent(row)?);
    }

    Ok(agents)
}

pub async fn find_user_agent(pool: &PgPool, id: &str, user_id: &str) -> Result<Option<UserAgent>> {
    let sql = format!("SELECT {SELECT_COLS} FROM user_agents WHERE id = $1 AND user_id = $2");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find user agent")?;

    match row {
        Some(row) => Ok(Some(row_to_user_agent(&row)?)),
        None => Ok(None),
    }
}

pub async fn find_user_agent_by_name(
    pool: &PgPool,
    name: &str,
    user_id: &str,
    account_id: &str,
) -> Result<Option<UserAgent>> {
    let sql = format!(
        "SELECT {SELECT_COLS} FROM user_agents WHERE name = $1 AND user_id = $2 AND account_id = $3"
    );
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(name)
        .bind(user_id)
        .bind(account_id)
        .fetch_optional(pool)
        .await
        .context("Failed to find user agent by name")?;

    match row {
        Some(row) => Ok(Some(row_to_user_agent(&row)?)),
        None => Ok(None),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn create_user_agent(
    pool: &PgPool,
    user_id: &str,
    account_id: &str,
    name: &str,
    goal: &str,
    steps_json: &str,
    output_style: &str,
    config_json: &str,
) -> Result<UserAgent> {
    let id = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO user_agents (id, user_id, account_id, name, goal, steps_json, output_style, config_json) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
    )
    .bind(id.as_str())
    .bind(user_id)
    .bind(account_id)
    .bind(name)
    .bind(goal)
    .bind(steps_json)
    .bind(output_style)
    .bind(config_json)
    .execute(pool)
    .await
    .context("Failed to insert user agent")?;

    find_user_agent(pool, &id, user_id)
        .await?
        .context("User agent not found after insert")
}

#[allow(clippy::too_many_arguments)]
pub async fn update_user_agent(
    pool: &PgPool,
    id: &str,
    user_id: &str,
    name: Option<&str>,
    goal: Option<&str>,
    steps_json: Option<&str>,
    output_style: Option<&str>,
    config_json: Option<&str>,
) -> Result<UserAgent> {
    let mut sets = Vec::new();
    let mut params: Vec<String> = Vec::new();
    let mut idx = 1;

    if let Some(name) = name {
        sets.push(format!("name = ${idx}"));
        params.push(name.to_string());
        idx += 1;
    }
    if let Some(goal) = goal {
        sets.push(format!("goal = ${idx}"));
        params.push(goal.to_string());
        idx += 1;
    }
    if let Some(steps_json) = steps_json {
        sets.push(format!("steps_json = ${idx}"));
        params.push(steps_json.to_string());
        idx += 1;
    }
    if let Some(output_style) = output_style {
        sets.push(format!("output_style = ${idx}"));
        params.push(output_style.to_string());
        idx += 1;
    }
    if let Some(config_json) = config_json {
        sets.push(format!("config_json = ${idx}"));
        params.push(config_json.to_string());
        idx += 1;
    }

    if sets.is_empty() {
        return find_user_agent(pool, id, user_id)
            .await?
            .context("User agent not found");
    }

    sets.push("updated_at = now()".to_string());

    let sql = format!(
        "UPDATE user_agents SET {} WHERE id = ${} AND user_id = ${}",
        sets.join(", "),
        idx,
        idx + 1
    );

    let mut query = sqlx::query(sqlx::AssertSqlSafe(sql));
    for param in &params {
        query = query.bind(param);
    }
    query
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to update user agent")?;

    find_user_agent(pool, id, user_id)
        .await?
        .context("User agent not found after update")
}

pub async fn delete_user_agent(pool: &PgPool, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = sqlx::query("DELETE FROM user_agents WHERE id = $1 AND user_id = $2")
        .bind(id)
        .bind(user_id)
        .execute(pool)
        .await
        .context("Failed to delete user agent")?
        .rows_affected();

    Ok(rows_affected > 0)
}
