use postgres::{Client, NoTls};
use serde_json::Value;

use crate::langgraph_rs::store::base::{
    EmbeddingVector, NamespacePath, Store, StoreError, StoreItem, StoreListQuery, StoreScoredItem,
    StoreSearchQuery, StoreVectorQuery, namespace_matches_prefix, now_timestamp_string,
    vector_score,
};

#[derive(Debug, Clone)]
pub struct PostgresStore {
    connection_string: String,
}

impl PostgresStore {
    pub fn new(connection_string: impl Into<String>) -> Result<Self, StoreError> {
        let store = Self {
            connection_string: connection_string.into(),
        };
        let mut client = store.open_client()?;
        Self::initialize_schema(&mut client)?;
        Ok(store)
    }

    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    fn open_client(&self) -> Result<Client, StoreError> {
        Client::connect(&self.connection_string, NoTls).map_err(|err| {
            StoreError::storage(format!(
                "failed to connect to postgres '{}': {err}",
                self.connection_string
            ))
        })
    }

    fn initialize_schema(client: &mut Client) -> Result<(), StoreError> {
        client
            .batch_execute(
                r#"
                CREATE TABLE IF NOT EXISTS store_items (
                    namespace_json TEXT NOT NULL,
                    item_key TEXT NOT NULL,
                    value_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    metadata_json TEXT NOT NULL DEFAULT '{}',
                    PRIMARY KEY (namespace_json, item_key)
                );

                CREATE INDEX IF NOT EXISTS idx_store_items_namespace
                    ON store_items (namespace_json, item_key);

                CREATE TABLE IF NOT EXISTS store_embeddings (
                    namespace_json TEXT NOT NULL,
                    item_key TEXT NOT NULL,
                    embedding_json TEXT NOT NULL,
                    dimension INTEGER NOT NULL,
                    updated_at TEXT NOT NULL,
                    PRIMARY KEY (namespace_json, item_key),
                    FOREIGN KEY (namespace_json, item_key)
                        REFERENCES store_items (namespace_json, item_key)
                        ON DELETE CASCADE
                );

                CREATE INDEX IF NOT EXISTS idx_store_embeddings_namespace
                    ON store_embeddings (namespace_json, item_key);
                "#,
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to initialize postgres schema: {err}"))
            })
    }

    fn load_item(
        namespace_json: &str,
        key: &str,
        value_json: &str,
        created_at: &str,
        updated_at: &str,
        metadata_json: &str,
    ) -> Result<StoreItem, StoreError> {
        let namespace: NamespacePath = serde_json::from_str(namespace_json).map_err(|err| {
            StoreError::serialization(format!(
                "failed to deserialize namespace for key '{key}': {err}"
            ))
        })?;
        let value: Value = serde_json::from_str(value_json).map_err(|err| {
            StoreError::serialization(format!(
                "failed to deserialize value for key '{key}': {err}"
            ))
        })?;
        let metadata = serde_json::from_str(metadata_json).map_err(|err| {
            StoreError::serialization(format!(
                "failed to deserialize metadata for key '{key}': {err}"
            ))
        })?;

        Ok(StoreItem {
            namespace,
            key: key.to_owned(),
            value,
            created_at: created_at.to_owned(),
            updated_at: updated_at.to_owned(),
            metadata,
        })
    }

    fn load_embedding(key: &str, embedding_json: &str) -> Result<EmbeddingVector, StoreError> {
        let embedding: EmbeddingVector = serde_json::from_str(embedding_json).map_err(|err| {
            StoreError::serialization(format!(
                "failed to deserialize embedding for key '{key}': {err}"
            ))
        })?;
        validate_embedding(&embedding)?;
        Ok(embedding)
    }
}

impl Store for PostgresStore {
    fn put(
        &self,
        namespace: &NamespacePath,
        key: &str,
        value: Value,
    ) -> Result<StoreItem, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let namespace_json = serde_json::to_string(namespace).map_err(|err| {
            StoreError::serialization(format!("failed to serialize namespace for put: {err}"))
        })?;
        let value_json = serde_json::to_string(&value).map_err(|err| {
            StoreError::serialization(format!("failed to serialize value for put: {err}"))
        })?;

        let mut client = self.open_client()?;
        let existing = client
            .query_opt(
                r#"
                SELECT created_at, metadata_json
                FROM store_items
                WHERE namespace_json = $1
                  AND item_key = $2
                "#,
                &[&namespace_json, &key],
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to query existing postgres row: {err}"))
            })?;

        let now = now_timestamp_string();
        let (created_at, metadata_json) = existing
            .map(|row| {
                let created_at: String = row.get(0);
                let metadata_json: String = row.get(1);
                (created_at, metadata_json)
            })
            .unwrap_or((now.clone(), "{}".to_owned()));

        client
            .execute(
                r#"
                INSERT INTO store_items (
                    namespace_json, item_key, value_json, created_at, updated_at, metadata_json
                ) VALUES ($1, $2, $3, $4, $5, $6)
                ON CONFLICT (namespace_json, item_key) DO UPDATE SET
                    value_json = EXCLUDED.value_json,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &namespace_json,
                    &key,
                    &value_json,
                    &created_at,
                    &now,
                    &metadata_json,
                ],
            )
            .map_err(|err| StoreError::storage(format!("failed to upsert postgres row: {err}")))?;

        let metadata = serde_json::from_str(&metadata_json).map_err(|err| {
            StoreError::serialization(format!("failed to deserialize stored metadata: {err}"))
        })?;

        Ok(StoreItem {
            namespace: namespace.clone(),
            key: key.to_owned(),
            value,
            created_at,
            updated_at: now,
            metadata,
        })
    }

    fn get(&self, namespace: &NamespacePath, key: &str) -> Result<Option<StoreItem>, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let namespace_json = serde_json::to_string(namespace).map_err(|err| {
            StoreError::serialization(format!("failed to serialize namespace for get: {err}"))
        })?;
        let mut client = self.open_client()?;
        let row = client
            .query_opt(
                r#"
                SELECT value_json, created_at, updated_at, metadata_json
                FROM store_items
                WHERE namespace_json = $1
                  AND item_key = $2
                "#,
                &[&namespace_json, &key],
            )
            .map_err(|err| StoreError::storage(format!("failed to query postgres row: {err}")))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let value_json: String = row.get(0);
        let created_at: String = row.get(1);
        let updated_at: String = row.get(2);
        let metadata_json: String = row.get(3);

        Ok(Some(Self::load_item(
            &namespace_json,
            key,
            &value_json,
            &created_at,
            &updated_at,
            &metadata_json,
        )?))
    }

    fn delete(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let namespace_json = serde_json::to_string(namespace).map_err(|err| {
            StoreError::serialization(format!("failed to serialize namespace for delete: {err}"))
        })?;
        let mut client = self.open_client()?;

        let rows = client
            .execute(
                r#"
                DELETE FROM store_items
                WHERE namespace_json = $1
                  AND item_key = $2
                "#,
                &[&namespace_json, &key],
            )
            .map_err(|err| StoreError::storage(format!("failed to delete postgres row: {err}")))?;

        Ok(rows > 0)
    }

    fn list(&self, query: &StoreListQuery) -> Result<Vec<StoreItem>, StoreError> {
        if let Some(namespace) = &query.namespace {
            validate_namespace(namespace)?;
        }
        if let Some(prefix) = &query.namespace_prefix {
            validate_namespace(prefix)?;
        }

        let limit = query.limit.unwrap_or(usize::MAX);
        let mut client = self.open_client()?;
        let rows = client
            .query(
                r#"
                SELECT namespace_json, item_key, value_json, created_at, updated_at, metadata_json
                FROM store_items
                ORDER BY namespace_json ASC, item_key ASC
                "#,
                &[],
            )
            .map_err(|err| StoreError::storage(format!("failed to query postgres list: {err}")))?;

        let mut items = Vec::new();
        for row in rows {
            if items.len() >= limit {
                break;
            }

            let namespace_json: String = row.get(0);
            let key: String = row.get(1);
            let value_json: String = row.get(2);
            let created_at: String = row.get(3);
            let updated_at: String = row.get(4);
            let metadata_json: String = row.get(5);

            let item = Self::load_item(
                &namespace_json,
                &key,
                &value_json,
                &created_at,
                &updated_at,
                &metadata_json,
            )?;

            if let Some(namespace) = &query.namespace
                && &item.namespace != namespace
            {
                continue;
            }
            if let Some(prefix) = &query.namespace_prefix
                && !namespace_matches_prefix(&item.namespace, prefix)
            {
                continue;
            }

            items.push(item);
        }

        Ok(items)
    }

    fn search(&self, query: &StoreSearchQuery) -> Result<Vec<StoreItem>, StoreError> {
        let needle = query.query.trim();
        if needle.is_empty() {
            return Err(StoreError::invalid_input(
                "search query cannot be empty or whitespace",
            ));
        }
        if let Some(prefix) = &query.namespace_prefix {
            validate_namespace(prefix)?;
        }

        let needle = needle.to_lowercase();
        let limit = query.limit.unwrap_or(usize::MAX);
        let mut client = self.open_client()?;
        let rows = client
            .query(
                r#"
                SELECT namespace_json, item_key, value_json, created_at, updated_at, metadata_json
                FROM store_items
                ORDER BY namespace_json ASC, item_key ASC
                "#,
                &[],
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to query postgres search: {err}"))
            })?;

        let mut items = Vec::new();
        for row in rows {
            if items.len() >= limit {
                break;
            }

            let namespace_json: String = row.get(0);
            let key: String = row.get(1);
            let value_json: String = row.get(2);
            let created_at: String = row.get(3);
            let updated_at: String = row.get(4);
            let metadata_json: String = row.get(5);

            let item = Self::load_item(
                &namespace_json,
                &key,
                &value_json,
                &created_at,
                &updated_at,
                &metadata_json,
            )?;

            if let Some(prefix) = &query.namespace_prefix
                && !namespace_matches_prefix(&item.namespace, prefix)
            {
                continue;
            }

            let key_match = item.key.to_lowercase().contains(&needle);
            let value_match = item.value.to_string().to_lowercase().contains(&needle);
            if key_match || value_match {
                items.push(item);
            }
        }

        Ok(items)
    }

    fn put_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
        embedding: EmbeddingVector,
    ) -> Result<(), StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;
        validate_embedding(&embedding)?;

        let namespace_json = serde_json::to_string(namespace).map_err(|err| {
            StoreError::serialization(format!(
                "failed to serialize namespace for put_embedding: {err}"
            ))
        })?;
        let embedding_json = serde_json::to_string(&embedding).map_err(|err| {
            StoreError::serialization(format!(
                "failed to serialize embedding for put_embedding: {err}"
            ))
        })?;
        let dimension = i32::try_from(embedding.len())
            .map_err(|_| StoreError::invalid_input("embedding dimension is too large"))?;

        let mut client = self.open_client()?;
        let exists = client
            .query_one(
                r#"
                SELECT EXISTS(
                    SELECT 1
                    FROM store_items
                    WHERE namespace_json = $1
                      AND item_key = $2
                )
                "#,
                &[&namespace_json, &key],
            )
            .map_err(|err| {
                StoreError::storage(format!(
                    "failed to check postgres store row existence: {err}"
                ))
            })?
            .get::<_, bool>(0);
        if !exists {
            return Err(StoreError::invalid_input(
                "cannot set embedding for missing namespace/key",
            ));
        }

        client
            .execute(
                r#"
                INSERT INTO store_embeddings (
                    namespace_json, item_key, embedding_json, dimension, updated_at
                ) VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (namespace_json, item_key) DO UPDATE SET
                    embedding_json = EXCLUDED.embedding_json,
                    dimension = EXCLUDED.dimension,
                    updated_at = EXCLUDED.updated_at
                "#,
                &[
                    &namespace_json,
                    &key,
                    &embedding_json,
                    &dimension,
                    &now_timestamp_string(),
                ],
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to upsert postgres embedding row: {err}"))
            })?;

        Ok(())
    }

    fn get_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
    ) -> Result<Option<EmbeddingVector>, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let namespace_json = serde_json::to_string(namespace).map_err(|err| {
            StoreError::serialization(format!(
                "failed to serialize namespace for get_embedding: {err}"
            ))
        })?;
        let mut client = self.open_client()?;
        let row = client
            .query_opt(
                r#"
                SELECT embedding_json
                FROM store_embeddings
                WHERE namespace_json = $1
                  AND item_key = $2
                "#,
                &[&namespace_json, &key],
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to query postgres embedding row: {err}"))
            })?;

        let Some(row) = row else {
            return Ok(None);
        };
        let embedding_json: String = row.get(0);
        Ok(Some(Self::load_embedding(key, &embedding_json)?))
    }

    fn delete_embedding(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let namespace_json = serde_json::to_string(namespace).map_err(|err| {
            StoreError::serialization(format!(
                "failed to serialize namespace for delete_embedding: {err}"
            ))
        })?;
        let mut client = self.open_client()?;
        let rows = client
            .execute(
                r#"
                DELETE FROM store_embeddings
                WHERE namespace_json = $1
                  AND item_key = $2
                "#,
                &[&namespace_json, &key],
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to delete postgres embedding row: {err}"))
            })?;
        Ok(rows > 0)
    }

    fn vector_search(&self, query: &StoreVectorQuery) -> Result<Vec<StoreScoredItem>, StoreError> {
        validate_embedding(&query.embedding)?;
        if let Some(prefix) = &query.namespace_prefix {
            validate_namespace(prefix)?;
        }

        let limit = query.limit.unwrap_or(usize::MAX);
        let mut client = self.open_client()?;
        let rows = client
            .query(
                r#"
                SELECT
                    si.namespace_json,
                    si.item_key,
                    si.value_json,
                    si.created_at,
                    si.updated_at,
                    si.metadata_json,
                    se.embedding_json
                FROM store_embeddings se
                INNER JOIN store_items si
                    ON si.namespace_json = se.namespace_json
                   AND si.item_key = se.item_key
                ORDER BY si.namespace_json ASC, si.item_key ASC
                "#,
                &[],
            )
            .map_err(|err| {
                StoreError::storage(format!("failed to query postgres vector search: {err}"))
            })?;

        let mut matched = Vec::<StoreScoredItem>::new();
        for row in rows {
            let namespace_json: String = row.get(0);
            let key: String = row.get(1);
            let value_json: String = row.get(2);
            let created_at: String = row.get(3);
            let updated_at: String = row.get(4);
            let metadata_json: String = row.get(5);
            let embedding_json: String = row.get(6);

            let item = Self::load_item(
                &namespace_json,
                &key,
                &value_json,
                &created_at,
                &updated_at,
                &metadata_json,
            )?;
            if let Some(prefix) = &query.namespace_prefix
                && !namespace_matches_prefix(&item.namespace, prefix)
            {
                continue;
            }

            let embedding = Self::load_embedding(&key, &embedding_json)?;
            let Some(score) = vector_score(&query.embedding, &embedding, query.metric) else {
                continue;
            };
            if let Some(min_score) = query.min_score
                && score < min_score
            {
                continue;
            }
            matched.push(StoreScoredItem::new(item, score));
        }

        matched.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        if matched.len() > limit {
            matched.truncate(limit);
        }
        Ok(matched)
    }
}

fn validate_namespace(namespace: &[String]) -> Result<(), StoreError> {
    if namespace.iter().any(|segment| segment.is_empty()) {
        return Err(StoreError::invalid_input(
            "namespace segments cannot be empty strings",
        ));
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), StoreError> {
    if key.trim().is_empty() {
        return Err(StoreError::invalid_input(
            "key cannot be empty or whitespace",
        ));
    }
    Ok(())
}

fn validate_embedding(embedding: &[f32]) -> Result<(), StoreError> {
    if embedding.is_empty() {
        return Err(StoreError::invalid_input("embedding cannot be empty"));
    }
    if embedding.iter().any(|value| !value.is_finite()) {
        return Err(StoreError::invalid_input(
            "embedding values must be finite numbers",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::store::base::{Store, StoreSearchQuery, StoreVectorQuery};

    use super::PostgresStore;

    #[test]
    fn roundtrips_put_and_get_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let scope = format!("store-postgres-{}", uuid::Uuid::new_v4().simple());
        let store = PostgresStore::new(connection_string).unwrap();
        let namespace = vec![scope.clone(), "profile".to_owned()];
        store.put(&namespace, "name", json!("Ada")).unwrap();

        let loaded = store.get(&namespace, "name").unwrap().unwrap();
        assert_eq!(loaded.value, json!("Ada"));
        let _ = store.delete(&namespace, "name");
    }

    #[test]
    fn supports_search_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let scope = format!("store-postgres-search-{}", uuid::Uuid::new_v4().simple());
        let store = PostgresStore::new(connection_string).unwrap();
        let namespace = vec![scope.clone(), "profile".to_owned()];
        store.put(&namespace, "city", json!("Lagos")).unwrap();

        let matches = store.search(&StoreSearchQuery::new("lag")).unwrap();
        assert!(
            matches
                .iter()
                .any(|item| item.namespace == namespace && item.key == "city")
        );
        let _ = store.delete(&namespace, "city");
    }

    #[test]
    fn supports_embedding_roundtrip_and_vector_search_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let scope = format!("store-postgres-vector-{}", uuid::Uuid::new_v4().simple());
        let store = PostgresStore::new(connection_string).unwrap();
        let namespace = vec![scope.clone(), "profile".to_owned()];
        store.put(&namespace, "a", json!({"text":"alpha"})).unwrap();
        store.put(&namespace, "b", json!({"text":"beta"})).unwrap();
        store
            .put_embedding(&namespace, "a", vec![1.0, 0.0])
            .unwrap();
        store
            .put_embedding(&namespace, "b", vec![0.0, 1.0])
            .unwrap();

        let loaded = store.get_embedding(&namespace, "a").unwrap().unwrap();
        assert_eq!(loaded, vec![1.0, 0.0]);

        let matches = store
            .vector_search(
                &StoreVectorQuery::new(vec![1.0, 0.0])
                    .with_namespace_prefix(namespace.clone())
                    .with_limit(1),
            )
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].item.key, "a");

        let _ = store.delete(&namespace, "a");
        let _ = store.delete(&namespace, "b");
    }
}
