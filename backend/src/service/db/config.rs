//! Environment-based Postgres schema partitioning.
//!
//! A single Postgres database is partitioned per environment using schemas, so
//! dev and prod data never collide without running a second database. The active
//! schema is chosen by the `POSTGRES_DATABASE` env var:
//!
//!   POSTGRES_DATABASE=dev   -> schema `tradstry_dev`
//!   POSTGRES_DATABASE=prod  -> schema `tradstry_prod`
//!   (unset / empty)         -> no override; uses the default `public` schema
//!
//! Every connection sets `search_path` to `"<schema>", public` so unqualified
//! DDL/queries are created in and read from the env schema, while extension types
//! (pgvector's `vector`, ParadeDB's `pg_search`) still resolve from `public`.

use anyhow::{Result, bail};

/// The Postgres schema for this environment, or `None` when `POSTGRES_DATABASE`
/// is unset/empty (meaning: use the database default `public` schema).
pub fn env_schema() -> Result<Option<String>> {
    match std::env::var("POSTGRES_DATABASE") {
        Ok(v) if !v.trim().is_empty() => {
            let v = v.trim().to_ascii_lowercase();
            // The value is interpolated into SET search_path / CREATE SCHEMA, so
            // restrict it to a safe identifier charset (no quoting/injection risk).
            if !v.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                bail!("POSTGRES_DATABASE must be [a-z0-9_], got {v:?}");
            }
            Ok(Some(format!("tradstry_{v}")))
        }
        _ => Ok(None),
    }
}

/// The `search_path` value to set per connection (env schema first, then
/// `public` as a fallback for extension types). `None` when unpartitioned.
pub fn search_path() -> Result<Option<String>> {
    Ok(env_schema()?.map(|s| format!("\"{s}\", public")))
}

/// Append a libpq `options=-c search_path=...` parameter to a connection URL,
/// for non-sqlx (`postgres`-crate) clients like the LangGraph savers. Returns the
/// URL unchanged when unpartitioned.
pub fn url_with_search_path(base: &str) -> Result<String> {
    match env_schema()? {
        None => Ok(base.to_string()),
        Some(schema) => {
            // URL-encoded: options=-c search_path=<schema>,public
            let opt = format!("options=-c%20search_path%3D{schema}%2Cpublic");
            let sep = if base.contains('?') { '&' } else { '?' };
            Ok(format!("{base}{sep}{opt}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Single test: these mutate the shared process env var, so they must not run
    // in parallel with each other.
    #[test]
    fn env_schema_and_url_options() {
        // unset -> no override
        unsafe { std::env::remove_var("POSTGRES_DATABASE") };
        assert_eq!(env_schema().unwrap(), None);
        assert_eq!(
            url_with_search_path("postgres://h/db").unwrap(),
            "postgres://h/db"
        );

        // bad identifier -> error
        unsafe { std::env::set_var("POSTGRES_DATABASE", "dev; DROP") };
        assert!(env_schema().is_err());

        // valid -> tradstry_<env> + URL option appended
        unsafe { std::env::set_var("POSTGRES_DATABASE", "dev") };
        assert_eq!(env_schema().unwrap().as_deref(), Some("tradstry_dev"));
        let u = url_with_search_path("postgres://h/db").unwrap();
        assert!(u.contains("options=-c%20search_path%3Dtradstry_dev%2Cpublic"));
        let u2 = url_with_search_path("postgres://h/db?sslmode=require").unwrap();
        assert!(u2.contains("&options="));

        unsafe { std::env::remove_var("POSTGRES_DATABASE") };
    }
}
