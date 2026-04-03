use anyhow::{Context, Result};
use libsql::Connection;
use log::info;
use std::collections::{HashMap, HashSet};

use super::tables::SCHEMA_SQL;

/// Bump this when you change SCHEMA_SQL and want the diff re-applied.
pub const SCHEMA_VERSION: &str = "1.3";

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run declarative schema migration if version has changed.
///
/// Parses SCHEMA_SQL to determine the desired schema, introspects the live DB,
/// then applies the minimal diff: create/drop tables, add/drop columns,
/// rename columns/tables, create/drop indexes and triggers.
pub async fn migrate(conn: &Connection) -> Result<()> {
    ensure_version_table(conn).await?;

    let applied = get_applied_version(conn).await?;
    if applied.as_deref() == Some(SCHEMA_VERSION) {
        info!("Schema is up to date at v{}", SCHEMA_VERSION);
        return Ok(());
    }

    info!(
        "Migrating schema to v{} (was: {})",
        SCHEMA_VERSION,
        applied.as_deref().unwrap_or("none")
    );

    // Parse rename directives from comments before executing anything
    let table_renames = parse_table_renames(SCHEMA_SQL);
    let column_renames = parse_column_renames(SCHEMA_SQL);

    // Step 1: Apply table renames first (so the rest of the diff sees correct names)
    for (old, new) in &table_renames {
        if table_exists(conn, old).await? {
            info!("Renaming table {} -> {}", old, new);
            conn.execute(
                &format!("ALTER TABLE \"{}\" RENAME TO \"{}\"", old, new),
                libsql::params![],
            )
            .await
            .with_context(|| format!("Failed to rename table {} -> {}", old, new))?;
        }
    }

    // Step 2: Apply column renames (before we diff columns)
    for ((table, old_col), new_col) in &column_renames {
        if table_exists(conn, table).await? && column_exists(conn, table, old_col).await? {
            info!("Renaming column {}.{} -> {}", table, old_col, new_col);
            conn.execute(
                &format!(
                    "ALTER TABLE \"{}\" RENAME COLUMN \"{}\" TO \"{}\"",
                    table, old_col, new_col
                ),
                libsql::params![],
            )
            .await
            .with_context(|| {
                format!(
                    "Failed to rename column {}.{} -> {}",
                    table, old_col, new_col
                )
            })?;
        }
    }

    // Step 3: Create tables first so column diffs can be applied before any
    // indexes or triggers reference newly added columns.
    let statements = split_sql_statements(SCHEMA_SQL);
    for stmt in &statements {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || !is_create_table_statement(trimmed) {
            continue;
        }
        conn.execute(trimmed, libsql::params![]).await?;
    }

    // Step 4: Diff columns — find columns to add or drop
    let desired_tables = parse_desired_tables(SCHEMA_SQL);
    let live_tables = get_live_tables(conn).await?;

    for (table_name, desired_cols) in &desired_tables {
        if let Some(live_cols) = live_tables.get(table_name.as_str()) {
            let live_col_names: HashSet<&str> = live_cols.iter().map(|c| c.name.as_str()).collect();
            let desired_col_names: HashSet<&str> =
                desired_cols.iter().map(|c| c.name.as_str()).collect();

            // Add new columns
            for col in desired_cols {
                if !live_col_names.contains(col.name.as_str()) {
                    let default_clause = if col.notnull && col.default.is_none() {
                        // NOT NULL without default — need a safe default for ALTER TABLE ADD
                        match col.col_type.to_uppercase().as_str() {
                            t if t.contains("INT") => " DEFAULT 0".to_string(),
                            t if t.contains("REAL") || t.contains("DECIMAL") => {
                                " DEFAULT 0.0".to_string()
                            }
                            t if t.contains("BOOL") => " DEFAULT false".to_string(),
                            t if t.contains("BLOB") => " DEFAULT X''".to_string(),
                            _ => " DEFAULT ''".to_string(),
                        }
                    } else if let Some(ref def) = col.default {
                        format!(" DEFAULT {}", def)
                    } else {
                        String::new()
                    };

                    let null_clause = if col.notnull { " NOT NULL" } else { "" };

                    let sql = format!(
                        "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}{}{}",
                        table_name, col.name, col.col_type, null_clause, default_clause
                    );
                    info!("Adding column: {}.{}", table_name, col.name);
                    conn.execute(&sql, libsql::params![])
                        .await
                        .with_context(|| {
                            format!("Failed to add column {}.{}", table_name, col.name)
                        })?;
                }
            }

            // Drop removed columns (via ALTER TABLE DROP COLUMN where safe,
            // otherwise rebuild the table)
            let cols_to_drop: Vec<&str> = live_col_names
                .difference(&desired_col_names)
                .copied()
                .collect();

            for col_name in cols_to_drop {
                info!("Dropping column: {}.{}", table_name, col_name);
                let drop_result = conn
                    .execute(
                        &format!(
                            "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
                            table_name, col_name
                        ),
                        libsql::params![],
                    )
                    .await;

                if drop_result.is_err() {
                    // Column is constrained (PK, index, FK, etc.) — rebuild the table
                    info!(
                        "Direct drop failed for {}.{}, rebuilding table",
                        table_name, col_name
                    );
                    rebuild_table_without_columns(conn, table_name, &[col_name], desired_cols)
                        .await
                        .with_context(|| {
                            format!(
                                "Failed to rebuild table {} to drop column {}",
                                table_name, col_name
                            )
                        })?;
                    break; // rebuild handles all drops at once
                }
            }
        }
    }

    // Step 5: Drop tables that no longer exist in SCHEMA_SQL
    let desired_table_names: HashSet<String> = desired_tables.keys().cloned().collect();
    let internal_tables: HashSet<String> = ["_schema_version".to_string()].into();

    for live_table in live_tables.keys() {
        if !desired_table_names.contains(live_table)
            && !internal_tables.contains(live_table)
            && !live_table.starts_with("sqlite_")
            && !live_table.starts_with("_litestream")
            && !live_table.starts_with("libsql_")
        {
            info!("Dropping removed table: {}", live_table);
            conn.execute(
                &format!("DROP TABLE IF EXISTS \"{}\"", live_table),
                libsql::params![],
            )
            .await?;
        }
    }

    // Step 6: Drop indexes/triggers not in desired schema
    drop_stale_indexes(conn, SCHEMA_SQL).await?;
    drop_stale_triggers(conn, SCHEMA_SQL).await?;

    // Step 7: Recreate desired indexes/triggers now that tables and columns
    // are in their final shape for this schema version.
    for stmt in &statements {
        let trimmed = stmt.trim();
        if trimmed.is_empty() || !is_create_index_or_trigger_statement(trimmed) {
            continue;
        }
        conn.execute(trimmed, libsql::params![]).await?;
    }

    // Record version
    conn.execute(
        "INSERT OR REPLACE INTO _schema_version (id, version) VALUES (1, ?)",
        libsql::params![SCHEMA_VERSION],
    )
    .await?;

    info!("Schema v{} applied successfully", SCHEMA_VERSION);
    Ok(())
}

// ---------------------------------------------------------------------------
// Version table
// ---------------------------------------------------------------------------

async fn ensure_version_table(conn: &Connection) -> Result<()> {
    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS _schema_version (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            version TEXT NOT NULL,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        )
        "#,
        libsql::params![],
    )
    .await?;
    Ok(())
}

async fn get_applied_version(conn: &Connection) -> Result<Option<String>> {
    let mut rows = conn
        .query(
            "SELECT version FROM _schema_version WHERE id = 1",
            libsql::params![],
        )
        .await?;

    if let Some(row) = rows.next().await? {
        Ok(Some(row.get::<String>(0)?))
    } else {
        Ok(None)
    }
}

// ---------------------------------------------------------------------------
// DB introspection helpers
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ColumnInfo {
    name: String,
    col_type: String,
    notnull: bool,
    default: Option<String>,
}

async fn table_exists(conn: &Connection, table: &str) -> Result<bool> {
    let mut rows = conn
        .query(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?",
            libsql::params![table],
        )
        .await?;
    Ok(rows.next().await?.is_some())
}

async fn column_exists(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let cols = get_table_columns(conn, table).await?;
    Ok(cols.iter().any(|c| c.name == column))
}

async fn get_table_columns(conn: &Connection, table: &str) -> Result<Vec<ColumnInfo>> {
    let mut rows = conn
        .query(
            &format!("PRAGMA table_info(\"{}\")", table),
            libsql::params![],
        )
        .await?;

    let mut cols = Vec::new();
    while let Some(row) = rows.next().await? {
        cols.push(ColumnInfo {
            name: row.get::<String>(1)?,
            col_type: row.get::<String>(2)?,
            notnull: row.get::<i64>(3)? != 0,
            default: row.get::<libsql::Value>(4).ok().and_then(|v| match v {
                libsql::Value::Null => None,
                libsql::Value::Text(s) => Some(s),
                libsql::Value::Integer(i) => Some(i.to_string()),
                libsql::Value::Real(f) => Some(f.to_string()),
                libsql::Value::Blob(_) => Some("X''".to_string()),
            }),
        });
    }
    Ok(cols)
}

async fn get_live_tables(conn: &Connection) -> Result<HashMap<String, Vec<ColumnInfo>>> {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            libsql::params![],
        )
        .await?;

    let mut tables = HashMap::new();
    let mut names = Vec::new();
    while let Some(row) = rows.next().await? {
        names.push(row.get::<String>(0)?);
    }

    for name in names {
        let cols = get_table_columns(conn, &name).await?;
        tables.insert(name, cols);
    }
    Ok(tables)
}

async fn get_live_index_names(conn: &Connection) -> Result<HashSet<String>> {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%'",
            libsql::params![],
        )
        .await?;

    let mut names = HashSet::new();
    while let Some(row) = rows.next().await? {
        names.insert(row.get::<String>(0)?);
    }
    Ok(names)
}

async fn get_live_trigger_names(conn: &Connection) -> Result<HashSet<String>> {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_master WHERE type='trigger'",
            libsql::params![],
        )
        .await?;

    let mut names = HashSet::new();
    while let Some(row) = rows.next().await? {
        names.insert(row.get::<String>(0)?);
    }
    Ok(names)
}

// ---------------------------------------------------------------------------
// SQL parsing helpers (extract desired state from SCHEMA_SQL)
// ---------------------------------------------------------------------------

/// Parse CREATE TABLE statements to extract table -> columns mapping.
fn parse_desired_tables(sql: &str) -> HashMap<String, Vec<ColumnInfo>> {
    let mut tables = HashMap::new();
    let statements = split_sql_statements(sql);

    for stmt in &statements {
        let upper = stmt.trim().to_uppercase();
        if !upper.starts_with("CREATE TABLE") {
            continue;
        }

        // Extract table name
        let name = extract_table_name(stmt);
        if name.is_empty() {
            continue;
        }

        // Extract columns from between the outermost parentheses
        if let Some(cols) = extract_columns_from_create(stmt) {
            tables.insert(name, cols);
        }
    }
    tables
}

fn extract_table_name(stmt: &str) -> String {
    // CREATE TABLE IF NOT EXISTS tablename (
    let upper = stmt.to_uppercase();
    let after_table = if let Some(pos) = upper.find("EXISTS") {
        &stmt[pos + 6..]
    } else if let Some(pos) = upper.find("TABLE") {
        &stmt[pos + 5..]
    } else {
        return String::new();
    };

    after_table
        .trim()
        .split(|c: char| c == '(' || c.is_whitespace())
        .next()
        .unwrap_or("")
        .trim_matches('"')
        .to_string()
}

fn extract_columns_from_create(stmt: &str) -> Option<Vec<ColumnInfo>> {
    // Find the opening paren after the table name
    let open = stmt.find('(')?;
    // Find the matching close paren
    let body = &stmt[open + 1..];
    let mut depth = 1i32;
    let mut end = 0;
    for (i, ch) in body.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = i;
                    break;
                }
            }
            _ => {}
        }
    }

    let columns_str = &body[..end];
    let mut cols = Vec::new();

    // Split by comma, but respect nested parens (for CHECK constraints etc.)
    for part in split_column_defs(columns_str) {
        let trimmed = part.trim();
        let upper = trimmed.to_uppercase();

        // Skip table-level constraints
        if upper.starts_with("PRIMARY KEY")
            || upper.starts_with("FOREIGN KEY")
            || upper.starts_with("UNIQUE")
            || upper.starts_with("CHECK")
            || upper.starts_with("CONSTRAINT")
        {
            continue;
        }

        // Parse: name TYPE [NOT NULL] [DEFAULT ...]
        if let Some(col) = parse_column_def(trimmed) {
            cols.push(col);
        }
    }

    Some(cols)
}

fn split_column_defs(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for ch in s.chars() {
        match ch {
            '(' => {
                depth += 1;
                current.push(ch);
            }
            ')' => {
                depth -= 1;
                current.push(ch);
            }
            ',' if depth == 0 => {
                parts.push(current.clone());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current);
    }
    parts
}

fn parse_column_def(def: &str) -> Option<ColumnInfo> {
    let tokens: Vec<&str> = def.split_whitespace().collect();
    if tokens.is_empty() {
        return None;
    }

    let name = tokens[0].trim_matches('"').to_string();
    // Skip if it looks like a keyword (table constraint that slipped through)
    if name.to_uppercase() == "PRIMARY"
        || name.to_uppercase() == "FOREIGN"
        || name.to_uppercase() == "UNIQUE"
        || name.to_uppercase() == "CHECK"
    {
        return None;
    }

    let col_type = tokens.get(1).copied().unwrap_or("TEXT").to_string();
    let upper_def = def.to_uppercase();
    let notnull = upper_def.contains("NOT NULL");

    let default = if let Some(pos) = upper_def.find("DEFAULT") {
        let after = def[pos + 7..].trim();
        // Grab the default value (handles expressions in parens)
        if after.starts_with('(') {
            // Find matching close paren
            let mut d = 0i32;
            let mut end = 0;
            for (i, ch) in after.char_indices() {
                match ch {
                    '(' => d += 1,
                    ')' => {
                        d -= 1;
                        if d == 0 {
                            end = i + 1;
                            break;
                        }
                    }
                    _ => {}
                }
            }
            Some(after[..end].to_string())
        } else {
            // Single token default
            Some(
                after
                    .split(|c: char| c.is_whitespace() || c == ',')
                    .next()
                    .unwrap_or("")
                    .to_string(),
            )
        }
    } else {
        None
    };

    Some(ColumnInfo {
        name,
        col_type,
        notnull,
        default,
    })
}

/// Parse `-- rename_table: old_name -> new_name` directives
fn parse_table_renames(sql: &str) -> Vec<(String, String)> {
    let mut renames = Vec::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("-- rename_table:") {
            if let Some((old, new)) = rest.split_once("->") {
                renames.push((old.trim().to_string(), new.trim().to_string()));
            }
        }
    }
    renames
}

/// Parse `-- rename: table.old_col -> new_col` directives
fn parse_column_renames(sql: &str) -> Vec<((String, String), String)> {
    let mut renames = Vec::new();
    for line in sql.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("-- rename:") {
            if let Some((left, new_col)) = rest.split_once("->") {
                let left = left.trim();
                if let Some((table, old_col)) = left.split_once('.') {
                    renames.push((
                        (table.trim().to_string(), old_col.trim().to_string()),
                        new_col.trim().to_string(),
                    ));
                }
            }
        }
    }
    renames
}

/// Extract index names from SCHEMA_SQL
fn parse_desired_index_names(sql: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let statements = split_sql_statements(sql);
    for stmt in &statements {
        let upper = stmt.trim().to_uppercase();
        if upper.starts_with("CREATE INDEX") || upper.starts_with("CREATE UNIQUE INDEX") {
            // CREATE [UNIQUE] INDEX [IF NOT EXISTS] name ON ...
            let after_exists = if let Some(pos) = upper.find("EXISTS") {
                &stmt[pos + 6..]
            } else if upper.starts_with("CREATE UNIQUE INDEX") {
                &stmt[19..]
            } else {
                &stmt[12..]
            };
            if let Some(name) = after_exists
                .trim()
                .split(|c: char| c.is_whitespace())
                .next()
            {
                names.insert(name.trim_matches('"').to_string());
            }
        }
    }
    names
}

/// Extract trigger names from SCHEMA_SQL
fn parse_desired_trigger_names(sql: &str) -> HashSet<String> {
    let mut names = HashSet::new();
    let statements = split_sql_statements(sql);
    for stmt in &statements {
        let upper = stmt.trim().to_uppercase();
        if upper.starts_with("CREATE TRIGGER") {
            let after_exists = if let Some(pos) = upper.find("EXISTS") {
                &stmt[pos + 6..]
            } else {
                &stmt[14..]
            };
            if let Some(name) = after_exists
                .trim()
                .split(|c: char| c.is_whitespace() || c == '\n')
                .next()
            {
                names.insert(name.trim_matches('"').to_string());
            }
        }
    }
    names
}

fn is_create_table_statement(sql: &str) -> bool {
    let upper = sql.trim().to_uppercase();
    upper.starts_with("CREATE TABLE")
}

fn is_create_index_or_trigger_statement(sql: &str) -> bool {
    let upper = sql.trim().to_uppercase();
    upper.starts_with("CREATE INDEX")
        || upper.starts_with("CREATE UNIQUE INDEX")
        || upper.starts_with("CREATE TRIGGER")
}

async fn drop_stale_indexes(conn: &Connection, schema_sql: &str) -> Result<()> {
    let desired = parse_desired_index_names(schema_sql);
    let live = get_live_index_names(conn).await?;

    for name in &live {
        if !desired.contains(name) && !name.starts_with("sqlite_") {
            info!("Dropping stale index: {}", name);
            conn.execute(
                &format!("DROP INDEX IF EXISTS \"{}\"", name),
                libsql::params![],
            )
            .await?;
        }
    }
    Ok(())
}

async fn drop_stale_triggers(conn: &Connection, schema_sql: &str) -> Result<()> {
    let desired = parse_desired_trigger_names(schema_sql);
    let live = get_live_trigger_names(conn).await?;

    for name in &live {
        if !desired.contains(name) {
            info!("Dropping stale trigger: {}", name);
            conn.execute(
                &format!("DROP TRIGGER IF EXISTS \"{}\"", name),
                libsql::params![],
            )
            .await?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Table rebuild (SQLite 12-step procedure for dropping constrained columns)
// ---------------------------------------------------------------------------

async fn rebuild_table_without_columns(
    conn: &Connection,
    table: &str,
    drop_cols: &[&str],
    desired_cols: &[ColumnInfo],
) -> Result<()> {
    let keep_cols: Vec<&ColumnInfo> = desired_cols
        .iter()
        .filter(|c| !drop_cols.contains(&c.name.as_str()))
        .collect();

    let col_list: String = keep_cols
        .iter()
        .map(|c| format!("\"{}\"", c.name))
        .collect::<Vec<_>>()
        .join(", ");

    let tmp = format!("_rebuild_{}", table);

    // 1. Create temp table with desired schema
    let col_defs: String = keep_cols
        .iter()
        .map(|c| {
            let mut def = format!("\"{}\" {}", c.name, c.col_type);
            if c.notnull {
                def.push_str(" NOT NULL");
            }
            if let Some(ref d) = c.default {
                def.push_str(&format!(" DEFAULT {}", d));
            }
            def
        })
        .collect::<Vec<_>>()
        .join(", ");

    conn.execute(
        &format!("CREATE TABLE \"{}\" ({})", tmp, col_defs),
        libsql::params![],
    )
    .await?;

    // 2. Copy data
    conn.execute(
        &format!(
            "INSERT INTO \"{}\" ({}) SELECT {} FROM \"{}\"",
            tmp, col_list, col_list, table
        ),
        libsql::params![],
    )
    .await?;

    // 3. Drop old table
    conn.execute(&format!("DROP TABLE \"{}\"", table), libsql::params![])
        .await?;

    // 4. Rename temp to original
    conn.execute(
        &format!("ALTER TABLE \"{}\" RENAME TO \"{}\"", tmp, table),
        libsql::params![],
    )
    .await?;

    info!("Rebuilt table {} (dropped columns: {:?})", table, drop_cols);
    Ok(())
}

// ---------------------------------------------------------------------------
// SQL statement splitter (respects BEGIN...END blocks for triggers)
// ---------------------------------------------------------------------------

fn split_sql_statements(sql: &str) -> Vec<String> {
    let mut statements = Vec::new();
    let mut current = String::new();
    let mut depth = 0i32;

    for line in sql.lines() {
        let trimmed_upper = line.trim().to_uppercase();

        if trimmed_upper.starts_with("BEGIN") {
            depth += 1;
        }

        current.push_str(line);
        current.push('\n');

        if trimmed_upper.starts_with("END") && depth > 0 {
            depth -= 1;
            if depth == 0 {
                let trimmed = current.trim().trim_end_matches(';').to_string();
                if !trimmed.is_empty() {
                    statements.push(trimmed);
                }
                current.clear();
            }
        } else if depth == 0 && line.trim().ends_with(';') {
            let trimmed = current.trim().trim_end_matches(';').to_string();
            if !trimmed.is_empty() {
                statements.push(trimmed);
            }
            current.clear();
        }
    }

    let trimmed = current.trim().trim_end_matches(';').to_string();
    if !trimmed.is_empty() {
        statements.push(trimmed);
    }

    statements
}
