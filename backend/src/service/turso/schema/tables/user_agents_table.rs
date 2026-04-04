use anyhow::{Context, Result};
use async_graphql::SimpleObject;
use libsql::Connection;
use serde::{Deserialize, Serialize};
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

const SELECT_COLS: &str = "id, user_id, account_id, name, goal, steps_json, output_style, config_json, created_at, updated_at";

fn row_to_user_agent(row: &libsql::Row) -> Result<UserAgent> {
    Ok(UserAgent {
        id: row.get::<String>(0)?,
        user_id: row.get::<String>(1)?,
        account_id: row.get::<String>(2)?,
        name: row.get::<String>(3)?,
        goal: row.get::<String>(4)?,
        steps_json: row.get::<String>(5)?,
        output_style: row.get::<String>(6)?,
        config_json: row.get::<String>(7)?,
        created_at: row.get::<String>(8)?,
        updated_at: row.get::<String>(9)?,
    })
}

pub async fn list_user_agents(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
) -> Result<Vec<UserAgent>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM user_agents WHERE user_id = ?1 AND account_id = ?2 ORDER BY created_at DESC"
            ),
            libsql::params![user_id, account_id],
        )
        .await
        .context("Failed to list user agents")?;

    let mut agents = Vec::new();
    while let Some(row) = rows.next().await? {
        agents.push(row_to_user_agent(&row)?);
    }

    Ok(agents)
}

pub async fn find_user_agent(
    conn: &Connection,
    id: &str,
    user_id: &str,
) -> Result<Option<UserAgent>> {
    let mut rows = conn
        .query(
            &format!("SELECT {SELECT_COLS} FROM user_agents WHERE id = ?1 AND user_id = ?2"),
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to find user agent")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_user_agent(&row)?)),
        None => Ok(None),
    }
}

pub async fn find_user_agent_by_name(
    conn: &Connection,
    name: &str,
    user_id: &str,
    account_id: &str,
) -> Result<Option<UserAgent>> {
    let mut rows = conn
        .query(
            &format!(
                "SELECT {SELECT_COLS} FROM user_agents WHERE name = ?1 AND user_id = ?2 AND account_id = ?3"
            ),
            libsql::params![name, user_id, account_id],
        )
        .await
        .context("Failed to find user agent by name")?;

    match rows.next().await? {
        Some(row) => Ok(Some(row_to_user_agent(&row)?)),
        None => Ok(None),
    }
}

pub async fn create_user_agent(
    conn: &Connection,
    user_id: &str,
    account_id: &str,
    name: &str,
    goal: &str,
    steps_json: &str,
    output_style: &str,
    config_json: &str,
) -> Result<UserAgent> {
    let id = Uuid::new_v4().to_string();

    conn.execute(
        "INSERT INTO user_agents (id, user_id, account_id, name, goal, steps_json, output_style, config_json) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        libsql::params![
            id.as_str(),
            user_id,
            account_id,
            name,
            goal,
            steps_json,
            output_style,
            config_json,
        ],
    )
    .await
    .context("Failed to insert user agent")?;

    find_user_agent(conn, &id, user_id)
        .await?
        .context("User agent not found after insert")
}

pub async fn update_user_agent(
    conn: &Connection,
    id: &str,
    user_id: &str,
    name: Option<&str>,
    goal: Option<&str>,
    steps_json: Option<&str>,
    output_style: Option<&str>,
    config_json: Option<&str>,
) -> Result<UserAgent> {
    let mut sets = Vec::new();
    let mut params: Vec<libsql::Value> = Vec::new();
    let mut idx = 1;

    if let Some(name) = name {
        sets.push(format!("name = ?{idx}"));
        params.push(libsql::Value::Text(name.to_string()));
        idx += 1;
    }
    if let Some(goal) = goal {
        sets.push(format!("goal = ?{idx}"));
        params.push(libsql::Value::Text(goal.to_string()));
        idx += 1;
    }
    if let Some(steps_json) = steps_json {
        sets.push(format!("steps_json = ?{idx}"));
        params.push(libsql::Value::Text(steps_json.to_string()));
        idx += 1;
    }
    if let Some(output_style) = output_style {
        sets.push(format!("output_style = ?{idx}"));
        params.push(libsql::Value::Text(output_style.to_string()));
        idx += 1;
    }
    if let Some(config_json) = config_json {
        sets.push(format!("config_json = ?{idx}"));
        params.push(libsql::Value::Text(config_json.to_string()));
        idx += 1;
    }

    if sets.is_empty() {
        return find_user_agent(conn, id, user_id)
            .await?
            .context("User agent not found");
    }

    sets.push(format!("updated_at = datetime('now')"));
    params.push(libsql::Value::Text(id.to_string()));
    params.push(libsql::Value::Text(user_id.to_string()));

    let sql = format!(
        "UPDATE user_agents SET {} WHERE id = ?{} AND user_id = ?{}",
        sets.join(", "),
        idx,
        idx + 1
    );

    conn.execute(&sql, libsql::params_from_iter(params))
        .await
        .context("Failed to update user agent")?;

    find_user_agent(conn, id, user_id)
        .await?
        .context("User agent not found after update")
}

pub async fn delete_user_agent(conn: &Connection, id: &str, user_id: &str) -> Result<bool> {
    let rows_affected = conn
        .execute(
            "DELETE FROM user_agents WHERE id = ?1 AND user_id = ?2",
            libsql::params![id, user_id],
        )
        .await
        .context("Failed to delete user agent")?;

    Ok(rows_affected > 0)
}
