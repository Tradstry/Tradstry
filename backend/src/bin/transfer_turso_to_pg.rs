//! One-time data transfer: copy every table from a Turso/libsql database into
//! Postgres, parsing the legacy TEXT timestamps into real `TIMESTAMPTZ` values.
//!
//! Reads the SOURCE over the Turso HTTP `/v2/pipeline` API (so it needs no
//! libsql dependency and always sees the true primary), and writes to the
//! TARGET Postgres via sqlx. The TARGET schema is created/verified first via the
//! normal migration path, then each table is copied in FK-dependency order with
//! `ON CONFLICT DO NOTHING`, so the job is idempotent and re-runnable.
//!
//! Usage (env vars):
//!   SOURCE_DB_URL    libsql://... (Turso DB URL; auto-rewritten to https://)
//!   SOURCE_DB_TOKEN  Bearer token for the source DB
//!   POSTGRES_URL     target Postgres connection string
//!
//!   cargo run --bin transfer_turso_to_pg
//!
//! Point SOURCE_* at tradstry-dev for local testing, tradstry-prod for cutover.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, QueryBuilder, Row};
use std::collections::HashMap;

use tradstry_backend::service::db::schema::pg::migrate;
use tradstry_backend::service::db::util::parse_flexible_datetime;

/// Tables in FK-dependency order (parents before children).
const TABLES: &[&str] = &[
    "users",
    "accounts",
    "playbooks",
    "journal_entries",
    "brokerage_transactions",
    "brokerage_holdings",
    "brokerage_balances",
    "notebook_folders",
    "notebook_notes",
    "notebook_note_trades",
    "notebook::images",
    "ai_jobs",
    "ai_source_documents",
    "ai_artifacts",
    "ai_artifact_sources",
    "journal_brokerage_links",
    "user_agents",
    "user_prompts",
    "position_calculator_rules",
    "position_calculator_history",
    "position_calculator_plans",
    "tag_categories",
    "tags",
    "trade_tags",
];

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();

    let source_url = std::env::var("SOURCE_DB_URL")
        .context("SOURCE_DB_URL not set")?
        .replace("libsql://", "https://");
    let source_token = std::env::var("SOURCE_DB_TOKEN").context("SOURCE_DB_TOKEN not set")?;
    let pg_url = std::env::var("POSTGRES_URL").context("POSTGRES_URL not set")?;

    // Honor POSTGRES_DATABASE -> tradstry_<env> schema partitioning, so a prod
    // transfer writes into tradstry_prod (matching the running app).
    let schema = tradstry_backend::service::db::config::env_schema()?;
    let search_path = tradstry_backend::service::db::config::search_path()?;
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .after_connect(move |conn, _meta| {
            let schema = schema.clone();
            let search_path = search_path.clone();
            Box::pin(async move {
                use sqlx::Executor;
                if let Some(schema) = &schema {
                    conn.execute(sqlx::AssertSqlSafe(format!(
                        "CREATE SCHEMA IF NOT EXISTS \"{schema}\""
                    )))
                    .await?;
                }
                if let Some(sp) = &search_path {
                    conn.execute(sqlx::AssertSqlSafe(format!("SET search_path TO {sp}")))
                        .await?;
                }
                Ok(())
            })
        })
        .connect(&pg_url)
        .await
        .context("connect to target Postgres")?;

    let target_schema =
        tradstry_backend::service::db::config::env_schema()?.unwrap_or_else(|| "public".into());
    println!("Target schema: {target_schema}. Ensuring schema (running migrations)...");
    migrate(&pool).await?;

    let http = reqwest::Client::new();
    let pipeline_url = format!("{}/v2/pipeline", source_url.trim_end_matches('/'));

    let mut grand_total = 0usize;
    for table in TABLES {
        let pg_types = column_types(&pool, &target_schema, table).await?;
        if pg_types.is_empty() {
            println!("[{table}] no such target table, skipping");
            continue;
        }

        let src = match fetch_table(&http, &pipeline_url, &source_token, table).await {
            Ok(rows) => rows,
            Err(e) => {
                println!("[{table}] source read failed ({e}); skipping");
                continue;
            }
        };
        let SourceTable { columns, rows } = src;
        if rows.is_empty() {
            println!("[{table}] 0 rows");
            continue;
        }

        // Only copy columns present in BOTH the source and the target table.
        let shared: Vec<String> = columns
            .iter()
            .filter(|c| pg_types.contains_key(c.as_str()))
            .cloned()
            .collect();

        let mut inserted = 0usize;
        for row in &rows {
            let mut qb: QueryBuilder<sqlx::Postgres> = QueryBuilder::new("INSERT INTO ");
            qb.push(table);
            qb.push(" (");
            qb.push(shared.join(", "));
            qb.push(") VALUES (");
            for (i, col) in shared.iter().enumerate() {
                if i > 0 {
                    qb.push(", ");
                }
                let raw = row.get(col).cloned().flatten();
                let pg_type = pg_types
                    .get(col.as_str())
                    .map(String::as_str)
                    .unwrap_or("text");
                bind_value(&mut qb, pg_type, raw)?;
            }
            qb.push(") ON CONFLICT DO NOTHING");
            let affected = qb
                .build()
                .execute(&pool)
                .await
                .with_context(|| format!("insert into {table}"))?
                .rows_affected();
            inserted += affected as usize;
        }
        println!("[{table}] {inserted} inserted ({} source rows)", rows.len());
        grand_total += inserted;
    }

    println!("Done. {grand_total} rows inserted total.");
    Ok(())
}

/// Map of column_name -> Postgres data_type for a table in `schema`
/// (empty if no such table).
async fn column_types(pool: &PgPool, schema: &str, table: &str) -> Result<HashMap<String, String>> {
    let rows = sqlx::query(
        "SELECT column_name, data_type FROM information_schema.columns \
         WHERE table_schema = $1 AND table_name = $2",
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await?;
    let mut map = HashMap::new();
    for r in &rows {
        let name: String = r.try_get("column_name")?;
        let ty: String = r.try_get("data_type")?;
        map.insert(name, ty);
    }
    Ok(map)
}

struct SourceTable {
    columns: Vec<String>,
    rows: Vec<HashMap<String, Option<String>>>,
}

/// Query the Turso source for `SELECT * FROM <table>` and normalize each cell to
/// `Option<String>` (NULL -> None; integers/floats stringified).
async fn fetch_table(
    http: &reqwest::Client,
    pipeline_url: &str,
    token: &str,
    table: &str,
) -> Result<SourceTable> {
    let body = serde_json::json!({
        "requests": [
            {"type": "execute", "stmt": {"sql": format!("SELECT * FROM {table}")}},
            {"type": "close"}
        ]
    });
    let resp: serde_json::Value = http
        .post(pipeline_url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let result = resp
        .get("results")
        .and_then(|r| r.get(0))
        .and_then(|r| r.get("response"))
        .and_then(|r| r.get("result"))
        .ok_or_else(|| {
            let err = resp
                .get("results")
                .and_then(|r| r.get(0))
                .and_then(|r| r.get("error"))
                .map(|e| e.to_string())
                .unwrap_or_else(|| "no result".into());
            anyhow!("source query error: {err}")
        })?;

    let columns: Vec<String> = result
        .get("cols")
        .and_then(|c| c.as_array())
        .map(|arr| {
            arr.iter()
                .map(|c| {
                    c.get("name")
                        .and_then(|n| n.as_str())
                        .unwrap_or("")
                        .to_string()
                })
                .collect()
        })
        .unwrap_or_default();

    let mut rows = Vec::new();
    if let Some(arr) = result.get("rows").and_then(|r| r.as_array()) {
        for row in arr {
            let cells = row.as_array().cloned().unwrap_or_default();
            let mut map = HashMap::new();
            for (i, cell) in cells.iter().enumerate() {
                let col = columns.get(i).cloned().unwrap_or_default();
                map.insert(col, cell_to_string(cell));
            }
            rows.push(map);
        }
    }
    Ok(SourceTable { columns, rows })
}

/// A Turso/hrana cell is `{"type":"text|integer|float|null|blob","value":...}`.
fn cell_to_string(cell: &serde_json::Value) -> Option<String> {
    let ty = cell.get("type").and_then(|t| t.as_str()).unwrap_or("null");
    if ty == "null" {
        return None;
    }
    match cell.get("value") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        Some(serde_json::Value::Number(n)) => Some(n.to_string()),
        Some(serde_json::Value::Null) | None => None,
        Some(other) => Some(other.to_string()),
    }
}

/// Bind one normalized source value into the query, casting to the Rust type
/// that matches the target Postgres column type.
fn bind_value(
    qb: &mut QueryBuilder<sqlx::Postgres>,
    pg_type: &str,
    raw: Option<String>,
) -> Result<()> {
    // For non-text columns an empty string is a SQLite sentinel for "absent" —
    // treat it as NULL. (Text columns keep "" verbatim; some are NOT NULL DEFAULT '').
    let non_empty = raw.clone().filter(|s| !s.is_empty());
    match pg_type {
        "timestamp with time zone" | "timestamp without time zone" => {
            let v: Option<DateTime<Utc>> = match non_empty {
                Some(s) => Some(parse_flexible_datetime(&s)?),
                None => None,
            };
            qb.push_bind(v);
        }
        "boolean" => {
            let v: Option<bool> = non_empty
                .map(|s| s == "1" || s.eq_ignore_ascii_case("true") || s.eq_ignore_ascii_case("t"));
            qb.push_bind(v);
        }
        "bigint" | "integer" | "smallint" => {
            let v: Option<i64> = match non_empty {
                Some(s) => Some(
                    s.parse::<i64>()
                        .or_else(|_| s.parse::<f64>().map(|f| f as i64))
                        .with_context(|| format!("parse int from {s:?}"))?,
                ),
                None => None,
            };
            qb.push_bind(v);
        }
        "double precision" | "real" | "numeric" => {
            let v: Option<f64> = match non_empty {
                Some(s) => Some(
                    s.parse::<f64>()
                        .with_context(|| format!("parse f64 from {s:?}"))?,
                ),
                None => None,
            };
            qb.push_bind(v);
        }
        _ => {
            // text and everything else: keep value as-is (incl. "")
            qb.push_bind(raw);
        }
    }
    Ok(())
}
