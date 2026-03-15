use postgres::{Client, NoTls};
use serde_json::Value;

use crate::langgraph_rs::cache::base::{
    Cache, CacheError, CacheItem, CacheKey, CacheNamespace, CacheSetOptions, now_unix_millis,
};

#[derive(Debug, Clone)]
pub struct PostgresCache {
    connection_string: String,
}

impl PostgresCache {
    pub fn new(connection_string: impl Into<String>) -> Result<Self, CacheError> {
        let cache = Self {
            connection_string: connection_string.into(),
        };
        let mut client = cache.open_client()?;
        Self::initialize_schema(&mut client)?;
        Ok(cache)
    }

    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    fn open_client(&self) -> Result<Client, CacheError> {
        let mut client = Client::connect(&self.connection_string, NoTls).map_err(|err| {
            CacheError::storage(format!(
                "failed to connect to postgres '{}': {err}",
                self.connection_string
            ))
        })?;
        Self::initialize_schema(&mut client)?;
        Ok(client)
    }

    fn initialize_schema(client: &mut Client) -> Result<(), CacheError> {
        client
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS cache_entries (
                    namespace_json TEXT NOT NULL,
                    cache_key TEXT NOT NULL,
                    value_json TEXT NOT NULL,
                    created_at_millis BIGINT NOT NULL,
                    updated_at_millis BIGINT NOT NULL,
                    expires_at_millis BIGINT,
                    metadata_json TEXT NOT NULL DEFAULT '{}',
                    PRIMARY KEY (namespace_json, cache_key)
                );

                CREATE INDEX IF NOT EXISTS idx_cache_entries_expires_at
                    ON cache_entries (expires_at_millis);
                "#,
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to initialize postgres schema: {err}"))
            })
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

impl Cache for PostgresCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheItem>, CacheError> {
        validate_key(key)?;
        let namespace_json = Self::encode_namespace(&key.namespace)?;
        let mut client = self.open_client()?;

        let row = client
            .query_opt(
                r#"
                SELECT value_json, created_at_millis, updated_at_millis, expires_at_millis, metadata_json
                FROM cache_entries
                WHERE namespace_json = $1
                  AND cache_key = $2
                "#,
                &[&namespace_json, &key.key],
            )
            .map_err(|err| CacheError::storage(format!("failed to query postgres cache entry: {err}")))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let value_json: String = row.get(0);
        let created_at_i64: i64 = row.get(1);
        let updated_at_i64: i64 = row.get(2);
        let expires_at_i64: Option<i64> = row.get(3);
        let metadata_json: String = row.get(4);

        let created_at_millis = u64::try_from(created_at_i64)
            .map_err(|_| CacheError::storage("invalid created_at_millis in postgres cache row"))?;
        let updated_at_millis = u64::try_from(updated_at_i64)
            .map_err(|_| CacheError::storage("invalid updated_at_millis in postgres cache row"))?;
        let expires_at_millis = expires_at_i64
            .map(|value| {
                u64::try_from(value).map_err(|_| {
                    CacheError::storage("invalid expires_at_millis in postgres cache row")
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
            .map_err(|_| CacheError::storage("timestamp overflow for postgres cache row"))?;
        let expires_i64 = expires_at_millis
            .map(|millis| {
                i64::try_from(millis)
                    .map_err(|_| CacheError::storage("expires_at overflow for postgres cache row"))
            })
            .transpose()?;

        let mut client = self.open_client()?;
        let existing_created_at = client
            .query_opt(
                r#"
                SELECT created_at_millis
                FROM cache_entries
                WHERE namespace_json = $1
                  AND cache_key = $2
                "#,
                &[&namespace_json, &key.key],
            )
            .map_err(|err| {
                CacheError::storage(format!(
                    "failed to query existing postgres cache row: {err}"
                ))
            })?
            .map(|row| row.get::<_, i64>(0));

        let created_at_i64 = existing_created_at.unwrap_or(now_i64);

        client
            .execute(
                r#"
                INSERT INTO cache_entries (
                    namespace_json, cache_key, value_json, created_at_millis, updated_at_millis, expires_at_millis, metadata_json
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (namespace_json, cache_key) DO UPDATE SET
                    value_json = EXCLUDED.value_json,
                    updated_at_millis = EXCLUDED.updated_at_millis,
                    expires_at_millis = EXCLUDED.expires_at_millis,
                    metadata_json = EXCLUDED.metadata_json
                "#,
                &[
                    &namespace_json,
                    &key.key,
                    &value_json,
                    &created_at_i64,
                    &now_i64,
                    &expires_i64,
                    &metadata_json,
                ],
            )
            .map_err(|err| CacheError::storage(format!("failed to upsert postgres cache row: {err}")))?;

        Ok(CacheItem {
            cache_key: key.clone(),
            value,
            created_at_millis: u64::try_from(created_at_i64).map_err(|_| {
                CacheError::storage("invalid created_at_millis after postgres upsert")
            })?,
            updated_at_millis: now,
            expires_at_millis,
            metadata: options.metadata,
        })
    }

    fn delete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        validate_key(key)?;
        let namespace_json = Self::encode_namespace(&key.namespace)?;
        let mut client = self.open_client()?;
        let rows = client
            .execute(
                r#"
                DELETE FROM cache_entries
                WHERE namespace_json = $1
                  AND cache_key = $2
                "#,
                &[&namespace_json, &key.key],
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to delete postgres cache row: {err}"))
            })?;
        Ok(rows > 0)
    }

    fn clear_namespace(&self, namespace: &CacheNamespace) -> Result<usize, CacheError> {
        let namespace_json = Self::encode_namespace(namespace)?;
        let mut client = self.open_client()?;
        let rows = client
            .execute(
                r#"
                DELETE FROM cache_entries
                WHERE namespace_json = $1
                "#,
                &[&namespace_json],
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to clear postgres cache namespace: {err}"))
            })?;
        usize::try_from(rows)
            .map_err(|_| CacheError::storage("row count overflow in postgres clear_namespace"))
    }

    fn clear_all(&self) -> Result<usize, CacheError> {
        let mut client = self.open_client()?;
        let rows = client
            .execute("DELETE FROM cache_entries", &[])
            .map_err(|err| CacheError::storage(format!("failed to clear postgres cache: {err}")))?;
        usize::try_from(rows)
            .map_err(|_| CacheError::storage("row count overflow in postgres clear_all"))
    }

    fn prune_expired(&self) -> Result<usize, CacheError> {
        let mut client = self.open_client()?;
        let now_i64 = i64::try_from(now_unix_millis())
            .map_err(|_| CacheError::storage("timestamp overflow during postgres prune"))?;
        let rows = client
            .execute(
                r#"
                DELETE FROM cache_entries
                WHERE expires_at_millis IS NOT NULL
                  AND expires_at_millis <= $1
                "#,
                &[&now_i64],
            )
            .map_err(|err| {
                CacheError::storage(format!("failed to prune postgres cache entries: {err}"))
            })?;
        usize::try_from(rows)
            .map_err(|_| CacheError::storage("row count overflow in postgres prune_expired"))
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
    use serde_json::json;

    use crate::langgraph_rs::cache::base::{Cache, CacheKey, CacheSetOptions};

    use super::PostgresCache;

    fn delete_if_exists(cache: &PostgresCache, key: &CacheKey) {
        let _ = cache.delete(key);
    }

    #[test]
    fn roundtrips_cache_values_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let cache = PostgresCache::new(connection_string).unwrap();
        let key = CacheKey::new(
            vec![format!("cache-postgres-{}", uuid::Uuid::new_v4().simple())],
            "k1",
        );
        delete_if_exists(&cache, &key);

        cache
            .set(
                &key,
                json!({"v": 1}),
                CacheSetOptions::new().with_ttl_millis(1_000),
            )
            .unwrap();
        assert_eq!(cache.get(&key).unwrap().unwrap().value, json!({"v": 1}));
        let _ = cache.delete(&key);
    }

    #[test]
    fn expires_and_prunes_entries_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let cache = PostgresCache::new(connection_string).unwrap();
        let namespace = vec![format!(
            "cache-postgres-prune-{}",
            uuid::Uuid::new_v4().simple()
        )];
        let key_read = CacheKey::new(namespace.clone(), "k2_read");
        let key_prune = CacheKey::new(namespace, "k2_prune");
        delete_if_exists(&cache, &key_read);
        delete_if_exists(&cache, &key_prune);

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
        let _ = cache.delete(&key_read);
        let _ = cache.delete(&key_prune);
    }
}
