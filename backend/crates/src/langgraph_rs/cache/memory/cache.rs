use std::{
    collections::BTreeMap,
    sync::{RwLock, RwLockReadGuard, RwLockWriteGuard},
};

use serde_json::Value;

use crate::langgraph_rs::cache::base::{
    Cache, CacheError, CacheItem, CacheKey, CacheNamespace, CacheSetOptions, now_unix_millis,
};

#[derive(Debug, Default)]
struct InMemoryState {
    entries: BTreeMap<CacheKey, CacheItem>,
}

#[derive(Debug, Default)]
pub struct InMemoryCache {
    state: RwLock<InMemoryState>,
}

impl InMemoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn read_state(&self) -> Result<RwLockReadGuard<'_, InMemoryState>, CacheError> {
        self.state
            .read()
            .map_err(|_| CacheError::storage("in-memory cache read lock poisoned"))
    }

    fn write_state(&self) -> Result<RwLockWriteGuard<'_, InMemoryState>, CacheError> {
        self.state
            .write()
            .map_err(|_| CacheError::storage("in-memory cache write lock poisoned"))
    }
}

impl Cache for InMemoryCache {
    fn get(&self, key: &CacheKey) -> Result<Option<CacheItem>, CacheError> {
        validate_key(key)?;

        let state = self.read_state()?;
        let Some(item) = state.entries.get(key) else {
            return Ok(None);
        };
        if item.is_expired_at(now_unix_millis()) {
            return Ok(None);
        }
        Ok(Some(item.clone()))
    }

    fn set(
        &self,
        key: &CacheKey,
        value: Value,
        options: CacheSetOptions,
    ) -> Result<CacheItem, CacheError> {
        validate_key(key)?;

        let mut state = self.write_state()?;
        let now = now_unix_millis();
        let expires_at_millis = options.ttl_millis.map(|ttl| now.saturating_add(ttl));

        let item = match state.entries.get(key) {
            Some(existing) => CacheItem {
                cache_key: key.clone(),
                value,
                created_at_millis: existing.created_at_millis,
                updated_at_millis: now,
                expires_at_millis,
                metadata: options.metadata,
            },
            None => CacheItem {
                cache_key: key.clone(),
                value,
                created_at_millis: now,
                updated_at_millis: now,
                expires_at_millis,
                metadata: options.metadata,
            },
        };

        state.entries.insert(key.clone(), item.clone());
        Ok(item)
    }

    fn delete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        validate_key(key)?;
        let mut state = self.write_state()?;
        Ok(state.entries.remove(key).is_some())
    }

    fn clear_namespace(&self, namespace: &CacheNamespace) -> Result<usize, CacheError> {
        validate_namespace(namespace)?;

        let mut state = self.write_state()?;
        let before = state.entries.len();
        state
            .entries
            .retain(|cache_key, _| cache_key.namespace != *namespace);
        Ok(before.saturating_sub(state.entries.len()))
    }

    fn clear_all(&self) -> Result<usize, CacheError> {
        let mut state = self.write_state()?;
        let cleared = state.entries.len();
        state.entries.clear();
        Ok(cleared)
    }

    fn prune_expired(&self) -> Result<usize, CacheError> {
        let now = now_unix_millis();
        let mut state = self.write_state()?;
        let before = state.entries.len();
        state.entries.retain(|_, item| !item.is_expired_at(now));
        Ok(before.saturating_sub(state.entries.len()))
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

fn validate_key(cache_key: &CacheKey) -> Result<(), CacheError> {
    validate_namespace(&cache_key.namespace)?;
    if cache_key.key.trim().is_empty() {
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

    use super::InMemoryCache;

    #[test]
    fn set_and_get_roundtrip() {
        let cache = InMemoryCache::new();
        let key = CacheKey::new(vec!["thread".to_owned()], "k1");
        cache
            .set(
                &key,
                json!({"value": 1}),
                CacheSetOptions::new().with_ttl_millis(60_000),
            )
            .unwrap();

        let got = cache.get(&key).unwrap().unwrap();
        assert_eq!(got.value, json!({"value": 1}));
    }

    #[test]
    fn ttl_zero_behaves_as_immediately_expired() {
        let cache = InMemoryCache::new();
        let key = CacheKey::new(vec!["thread".to_owned()], "k2");
        cache
            .set(&key, json!("x"), CacheSetOptions::new().with_ttl_millis(0))
            .unwrap();

        let got = cache.get(&key).unwrap();
        assert!(got.is_none());
        let removed = cache.prune_expired().unwrap();
        assert_eq!(removed, 1);
    }

    #[test]
    fn clear_namespace_only_removes_target_namespace() {
        let cache = InMemoryCache::new();
        let a1 = CacheKey::new(vec!["a".to_owned()], "k1");
        let a2 = CacheKey::new(vec!["a".to_owned()], "k2");
        let b1 = CacheKey::new(vec!["b".to_owned()], "k1");

        cache
            .set(&a1, json!(1), CacheSetOptions::new().with_ttl_millis(1_000))
            .unwrap();
        cache
            .set(&a2, json!(2), CacheSetOptions::new().with_ttl_millis(1_000))
            .unwrap();
        cache
            .set(&b1, json!(3), CacheSetOptions::new().with_ttl_millis(1_000))
            .unwrap();

        let cleared = cache.clear_namespace(&vec!["a".to_owned()]).unwrap();
        assert_eq!(cleared, 2);
        assert!(cache.get(&a1).unwrap().is_none());
        assert!(cache.get(&a2).unwrap().is_none());
        assert!(cache.get(&b1).unwrap().is_some());
    }
}
