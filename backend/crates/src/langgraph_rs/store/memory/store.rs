use std::{
    collections::BTreeMap,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde_json::Value;

use crate::langgraph_rs::store::base::{
    EmbeddingVector, NamespacePath, Store, StoreError, StoreItem, StoreListQuery, StoreScoredItem,
    StoreSearchQuery, StoreVectorQuery, namespace_matches_prefix, now_timestamp_string,
    vector_score,
};

#[derive(Debug, Default)]
struct InMemoryState {
    by_namespace: BTreeMap<NamespacePath, BTreeMap<String, StoreItem>>,
    embeddings: BTreeMap<NamespacePath, BTreeMap<String, EmbeddingVector>>,
}

#[derive(Debug, Default)]
pub struct InMemoryStore {
    state: RwLock<InMemoryState>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn read_state(&self) -> Result<RwLockReadGuard<'_, InMemoryState>, StoreError> {
        self.state
            .read()
            .map_err(|_| StoreError::storage("in-memory store read lock poisoned"))
    }

    fn write_state(&self) -> Result<RwLockWriteGuard<'_, InMemoryState>, StoreError> {
        self.state
            .write()
            .map_err(|_| StoreError::storage("in-memory store write lock poisoned"))
    }
}

impl Store for InMemoryStore {
    fn put(
        &self,
        namespace: &NamespacePath,
        key: &str,
        value: Value,
    ) -> Result<StoreItem, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let mut state = self.write_state()?;
        let bucket = state.by_namespace.entry(namespace.clone()).or_default();

        if let Some(item) = bucket.get_mut(key) {
            item.value = value;
            item.updated_at = now_timestamp_string();
            return Ok(item.clone());
        }

        let item = StoreItem::new(namespace.clone(), key.to_owned(), value);
        bucket.insert(key.to_owned(), item.clone());
        Ok(item)
    }

    fn get(&self, namespace: &NamespacePath, key: &str) -> Result<Option<StoreItem>, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let state = self.read_state()?;
        Ok(state
            .by_namespace
            .get(namespace)
            .and_then(|bucket| bucket.get(key))
            .cloned())
    }

    fn delete(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let mut state = self.write_state()?;
        let removed = if let Some(bucket) = state.by_namespace.get_mut(namespace) {
            let removed = bucket.remove(key).is_some();
            if bucket.is_empty() {
                state.by_namespace.remove(namespace);
            }
            if let Some(embedding_bucket) = state.embeddings.get_mut(namespace) {
                embedding_bucket.remove(key);
                if embedding_bucket.is_empty() {
                    state.embeddings.remove(namespace);
                }
            }
            removed
        } else {
            false
        };
        Ok(removed)
    }

    fn list(&self, query: &StoreListQuery) -> Result<Vec<StoreItem>, StoreError> {
        if let Some(namespace) = &query.namespace {
            validate_namespace(namespace)?;
        }
        if let Some(namespace_prefix) = &query.namespace_prefix {
            validate_namespace(namespace_prefix)?;
        }

        let limit = query.limit.unwrap_or(usize::MAX);
        let state = self.read_state()?;
        let mut items = Vec::<StoreItem>::new();

        for (namespace, bucket) in &state.by_namespace {
            if let Some(filter_namespace) = &query.namespace
                && namespace != filter_namespace
            {
                continue;
            }
            if let Some(prefix) = &query.namespace_prefix
                && !namespace_matches_prefix(namespace, prefix)
            {
                continue;
            }

            for item in bucket.values() {
                items.push(item.clone());
                if items.len() >= limit {
                    return Ok(items);
                }
            }
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

        let limit = query.limit.unwrap_or(usize::MAX);
        let needle = needle.to_lowercase();

        let state = self.read_state()?;
        let mut matched = Vec::<StoreItem>::new();

        for (namespace, bucket) in &state.by_namespace {
            if let Some(prefix) = &query.namespace_prefix
                && !namespace_matches_prefix(namespace, prefix)
            {
                continue;
            }

            for item in bucket.values() {
                let key_match = item.key.to_lowercase().contains(&needle);
                let value_match = item.value.to_string().to_lowercase().contains(&needle);
                if key_match || value_match {
                    matched.push(item.clone());
                    if matched.len() >= limit {
                        return Ok(matched);
                    }
                }
            }
        }

        Ok(matched)
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

        let mut state = self.write_state()?;
        let Some(bucket) = state.by_namespace.get(namespace) else {
            return Err(StoreError::invalid_input(
                "cannot set embedding for missing namespace/key",
            ));
        };
        if !bucket.contains_key(key) {
            return Err(StoreError::invalid_input(
                "cannot set embedding for missing namespace/key",
            ));
        }

        state
            .embeddings
            .entry(namespace.clone())
            .or_default()
            .insert(key.to_owned(), embedding);
        Ok(())
    }

    fn get_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
    ) -> Result<Option<EmbeddingVector>, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let state = self.read_state()?;
        Ok(state
            .embeddings
            .get(namespace)
            .and_then(|bucket| bucket.get(key))
            .cloned())
    }

    fn delete_embedding(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        validate_namespace(namespace)?;
        validate_key(key)?;

        let mut state = self.write_state()?;
        let removed = if let Some(bucket) = state.embeddings.get_mut(namespace) {
            let removed = bucket.remove(key).is_some();
            if bucket.is_empty() {
                state.embeddings.remove(namespace);
            }
            removed
        } else {
            false
        };
        Ok(removed)
    }

    fn vector_search(&self, query: &StoreVectorQuery) -> Result<Vec<StoreScoredItem>, StoreError> {
        validate_embedding(&query.embedding)?;
        if let Some(prefix) = &query.namespace_prefix {
            validate_namespace(prefix)?;
        }

        let limit = query.limit.unwrap_or(usize::MAX);
        let state = self.read_state()?;
        let mut matched = Vec::<StoreScoredItem>::new();

        for (namespace, bucket) in &state.embeddings {
            if let Some(prefix) = &query.namespace_prefix
                && !namespace_matches_prefix(namespace, prefix)
            {
                continue;
            }
            let Some(items_bucket) = state.by_namespace.get(namespace) else {
                continue;
            };

            for (key, embedding) in bucket {
                let Some(item) = items_bucket.get(key) else {
                    continue;
                };
                let Some(score) = vector_score(&query.embedding, embedding, query.metric) else {
                    continue;
                };
                if let Some(min_score) = query.min_score
                    && score < min_score
                {
                    continue;
                }
                matched.push(StoreScoredItem::new(item.clone(), score));
            }
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

    use super::InMemoryStore;

    #[test]
    fn updates_existing_items_in_place() {
        let store = InMemoryStore::new();
        let ns = vec!["thread".to_owned(), "profile".to_owned()];

        let first = store.put(&ns, "prefs", json!({"theme":"light"})).unwrap();
        let second = store.put(&ns, "prefs", json!({"theme":"dark"})).unwrap();

        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.value, json!({"theme":"dark"}));
    }

    #[test]
    fn searches_key_and_value_text() {
        let store = InMemoryStore::new();
        let ns = vec!["thread".to_owned()];

        store.put(&ns, "name", json!("Alice")).unwrap();
        store.put(&ns, "city", json!("Lagos")).unwrap();

        let by_key = store.search(&StoreSearchQuery::new("nam")).unwrap();
        let by_value = store.search(&StoreSearchQuery::new("lag")).unwrap();

        assert_eq!(by_key.len(), 1);
        assert_eq!(by_key[0].key, "name");
        assert_eq!(by_value.len(), 1);
        assert_eq!(by_value[0].key, "city");
    }

    #[test]
    fn supports_vector_embedding_roundtrip_and_search() {
        let store = InMemoryStore::new();
        let ns = vec!["thread".to_owned(), "vectors".to_owned()];
        store.put(&ns, "a", json!({"text":"alpha"})).unwrap();
        store.put(&ns, "b", json!({"text":"beta"})).unwrap();

        store.put_embedding(&ns, "a", vec![1.0, 0.0]).unwrap();
        store.put_embedding(&ns, "b", vec![0.0, 1.0]).unwrap();

        let loaded = store.get_embedding(&ns, "a").unwrap().unwrap();
        assert_eq!(loaded, vec![1.0, 0.0]);

        let matches = store
            .vector_search(&StoreVectorQuery::new(vec![1.0, 0.0]).with_limit(1))
            .unwrap();
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].item.key, "a");
    }
}
