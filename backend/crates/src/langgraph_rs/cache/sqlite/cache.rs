use std::{
    fs,
    path::{Path, PathBuf},
};

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use crate::langgraph_rs::cache::base::{
    Cache, CacheError, CacheItem, CacheKey, CacheNamespace, CacheSetOptions, now_unix_millis,
};

#[derive(Debug, Clone)]
pub struct SqliteCache {
    db_path: PathBuf,
}

impl SqliteCache {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, CacheError> {
        let db_path = path.as_ref().to_path_buf();
        if let Some(parent) = db_path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent).map_err(|err| {
                    CacheError::storage(format!(
                        "failed to create sqlite parent directory '{}': {err}",
                        parent.display()
                    ))
                })?;
            }
        }

        let cache = Self { db_path };
        let conn = cache.open_connection()?;
        Self::initialize_schema(&conn)?;
        Ok(cache)
    }

    pub fn path(&self) -> &Path {
        &self.db_path
    }

    fn open_connection(&self) -> Result<Connection, CacheError> {
        let conn = Connection::open(&self.db_path).map_err(|err| {
            CacheError::storage(format!(
                "failed to open sqlite db '{}': {err}",
                self.db_path.display()
            ))
        })?;
        Self::initialize_schema(&conn)?;
        Ok(conn)
    }

    fn initialize_schema(conn: &Connection) -> Result<(), CacheError> {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS cache_entries (
                namespace_json TEXT NOT NULL,
                cache_key TEXT NOT NULL,
                value_json TEXT NOT NULL,
                created_at_millis INTEGER NOT NULL,
                updated_at_millis INTEGER NOT NULL,
                expires_at_millis INTEGER,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                PRIMARY KEY (namespace_json, cache_key)
            );

            CREATE INDEX IF NOT EXISTS idx_cache_entries_expires_at
                ON cache_entries (expires_at_millis);
            "#,
        )
        .map_err(|err| CacheError::storage(format!("failed to initialize sqlite schema: {err}")))
    }

    fn encode_namespace(namespace: &CacheNamespace) -> Result<String, CacheError> {
        validate_namespace(namespace)?;
        serde_json::to_string(namespace).map_err(|err| {
            CacheError::serialization(format!("failed to serialize cache namespace: {err}"))
        })
    }

    fn load_item(
        namespace_json: &str,
        key: &str,
        value_json: &str,
        created_at_millis: u64,
        updated_at_millis: u64,
        expires_at_millis: Option<u64>,
        metadata_json: &str,
    ) -> Result<CacheItem, CacheError> {
        let namespace = serde_json::from_str(namespace_json).map_err(|err| {
            CacheError::serialization(format!(
                "failed to deserialize cache namespace for key '{key}': {err}"
            ))
        })?;
        let value = serde_json::from_str(value_json).map_err(|err| {
            CacheError::serialization(format!(
                "failed to deserialize cache value for key '{key}': {err}"
            ))
        })?;
        let metadata = serde_json::from_str(metadata_json).map_err(|err| {
            CacheError::serialization(format!(
                "failed to deserialize cache metadata for key '{key}': {err}"
            ))
        })?;

        Ok(CacheItem {
            cache_key: CacheKey::new(namespace, key.to_owned()),
            value,
            created_at_millis,
            updated_at_millis,
            expires_at_millis,
            metadata,
        })
    }
}

impl Cache for SqliteCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheItem>, CacheError> {
        validate_key(key)?;

        let namespace_json = Self::encode_namespace(&key.namespace)?;
        let conn = self.open_connection()?;
        let row: Option<(String, i64, i64, Option<i64>, String)> = conn
            .query_row(
                r#"
                SELECT value_json, created_at_millis, updated_at_millis, expires_at_millis, metadata_json
                FROM cache_entries
                WHERE namespace_json = ?1
                  AND cache_key = ?2
                "#,
                params![&namespace_json, &key.key],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|err| CacheError::storage(format!("failed to query sqlite cache entry: {err}")))?;

        let Some((value_json, created_at, updated_at, expires_at, metadata_json)) = row else {
            return Ok(None);
        };

        let created_at_millis = u64::try_from(created_at)
            .map_err(|_| CacheError::storage("invalid created_at_millis in sqlite cache row"))?;
        let updated_at_millis = u64::try_from(updated_at)
            .map_err(|_| CacheError::storage("invalid updated_at_millis in sqlite cache row"))?;
        let expires_at_millis = expires_at
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    CacheError::storage("invalid expires_at_millis in sqlite cache row")
                })
            })
            .transpose()?;

        let item = Self::load_item(
            &namespace_json,
            &key.key,
            &value_json,
            created_at_millis,
            updated_at_millis,
            expires_at_millis,
            &metadata_json,
        )?;

        if item.is_expired_at(now_unix_millis()) {
            let _ = self.delete(key);
            return Ok(None);
        }

        Ok(Some(item))
    }

    fn set(
        &self,
        key: &CacheKey,
        value: Value,
        options: CacheSetOptions,
    ) -> Result<CacheItem, CacheError> {
        validate_key(key)?;

        let namespace_json = Self::encode_namespace(&key.namespace)?;
        let value_json = serde_json::to_string(&value).map_err(|err| {
            CacheError::serialization(format!("failed to serialize cache value: {err}"))
        })?;
        let metadata_json = serde_json::to_string(&options.metadata).map_err(|err| {
            CacheError::serialization(format!("failed to serialize cache metadata: {err}"))
        })?;

        let now = now_unix_millis();
        let expires_at_millis = options.ttl_millis.map(|ttl| now.saturating_add(ttl));
        let now_i64 = i64::try_from(now)
            .map_err(|_| CacheError::storage("timestamp overflow for sqlite cache row"))?;
        let expires_i64 = expires_at_millis
            .map(|millis| {
                i64::try_from(millis)
                    .map_err(|_| CacheError::storage("expires_at overflow for sqlite cache row"))
            })
            .transpose()?;

        let mut conn = self.open_connection()?;
        let tx = conn
            .transaction()
            .map_err(|err| CacheError::storage(format!("failed to open sqlite tx: {err}")))?;

        let existing_created_at: Option<i64> = tx
            .query_row(
                r#"
                SELECT created_at_millis
                FROM cache_entries
                WHERE namespace_json = ?1
                  AND cache_key = ?2
                "#,
                params![&namespace_json, &key.key],
                |row| row.get(0),
            )
            .optional()
            .map_err(|err| {
                CacheError::storage(format!(
                    "failed to query existing sqlite cache entry: {err}"
                ))
            })?;
        let created_at_i64 = existing_created_at.unwrap_or(now_i64);

        tx.execute(
            r#"
            INSERT INTO cache_entries (
                namespace_json, cache_key, value_json, created_at_millis, updated_at_millis, expires_at_millis, metadata_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(namespace_json, cache_key) DO UPDATE SET
                value_json = excluded.value_json,
                updated_at_millis = excluded.updated_at_millis,
                expires_at_millis = excluded.expires_at_millis,
                metadata_json = excluded.metadata_json
            "#,
            params![
                &namespace_json,
                &key.key,
                &value_json,
                created_at_i64,
                now_i64,
                expires_i64,
                &metadata_json
            ],
        )
        .map_err(|err| CacheError::storage(format!("failed to upsert sqlite cache entry: {err}")))?;

        tx.commit()
            .map_err(|err| CacheError::storage(format!("failed to commit sqlite tx: {err}")))?;

        Ok(CacheItem {
            cache_key: key.clone(),
            value,
            created_at_millis: u64::try_from(created_at_i64).map_err(|_| {
                CacheError::storage("invalid created_at_millis after sqlite upsert")
            })?,
            updated_at_millis: now,
            expires_at_millis,
            metadata: options.metadata,
        })
    }

    fn delete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        validate_key(key)?;
        let namespace_json = Self::encode_namespace(&key.namespace)?;
        let conn = self.open_connection()?;
        let rows = conn
            .execute(
                r#"
                DELETE FROM cache_entries
                WHERE namespace_json = ?1
                  AND cache_key = ?2
                "#,
                params![&namespace_json, &key.key],
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to delete sqlite cache entry: {err}"))
            })?;
        Ok(rows > 0)
    }

    fn clear_namespace(&self, namespace: &CacheNamespace) -> Result<usize, CacheError> {
        let namespace_json = Self::encode_namespace(namespace)?;
        let conn = self.open_connection()?;
        let rows = conn
            .execute(
                r#"
                DELETE FROM cache_entries
                WHERE namespace_json = ?1
                "#,
                params![&namespace_json],
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to clear sqlite cache namespace: {err}"))
            })?;
        Ok(rows)
    }

    fn clear_all(&self) -> Result<usize, CacheError> {
        let conn = self.open_connection()?;
        let rows = conn
            .execute("DELETE FROM cache_entries", [])
            .map_err(|err| CacheError::storage(format!("failed to clear sqlite cache: {err}")))?;
        Ok(rows)
    }

    fn prune_expired(&self) -> Result<usize, CacheError> {
        let conn = self.open_connection()?;
        let now_i64 = i64::try_from(now_unix_millis())
            .map_err(|_| CacheError::storage("timestamp overflow during sqlite prune"))?;
        let rows = conn
            .execute(
                r#"
                DELETE FROM cache_entries
                WHERE expires_at_millis IS NOT NULL
                  AND expires_at_millis <= ?1
                "#,
                params![now_i64],
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to prune sqlite cache entries: {err}"))
            })?;
        Ok(rows)
    }
}

fn validate_namespace(namespace: &[String]) -> Result<(), CacheError> {
    if namespace.iter().any(|segment| segment.is_empty()) {
        return Err(CacheError::invalid_input(
            "namespace segments cannot be empty strings",
        ));
    }
    Ok(())
}

fn validate_key(key: &CacheKey) -> Result<(), CacheError> {
    validate_namespace(&key.namespace)?;
    if key.key.trim().is_empty() {
        return Err(CacheError::invalid_input(
            "cache key cannot be empty or whitespace",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{env::temp_dir, fs};

    use serde_json::json;
    use uuid::Uuid;

    use crate::langgraph_rs::cache::base::{Cache, CacheKey, CacheSetOptions};

    use super::SqliteCache;

    #[test]
    fn roundtrips_cache_values_and_ttl() {
        let path = temp_dir().join(format!("langgraph_rs_cache_sqlite_{}.db", Uuid::new_v4()));
        let cache = SqliteCache::new(&path).unwrap();
        let key = CacheKey::new(vec!["thread".to_owned()], "k1");
        cache
            .set(
                &key,
                json!(1),
                CacheSetOptions::new().with_ttl_millis(1_000),
            )
            .unwrap();
        assert_eq!(cache.get(&key).unwrap().unwrap().value, json!(1));

        drop(cache);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn expires_and_prunes_entries() {
        let path = temp_dir().join(format!(
            "langgraph_rs_cache_sqlite_prune_{}.db",
            Uuid::new_v4()
        ));
        let cache = SqliteCache::new(&path).unwrap();
        let key_read = CacheKey::new(vec!["thread".to_owned()], "k2_read");
        let key_prune = CacheKey::new(vec!["thread".to_owned()], "k2_prune");
        cache
            .set(
                &key_read,
                json!("x"),
                CacheSetOptions::new().with_ttl_millis(0),
            )
            .unwrap();
        cache
            .set(
                &key_prune,
                json!("y"),
                CacheSetOptions::new().with_ttl_millis(0),
            )
            .unwrap();
        assert!(cache.get(&key_read).unwrap().is_none());
        assert_eq!(cache.prune_expired().unwrap(), 1);
        assert!(cache.get(&key_prune).unwrap().is_none());

        drop(cache);
        let _ = fs::remove_file(path);
    }
}
