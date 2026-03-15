#[cfg(test)]
mod tests {
    use std::{env::temp_dir, fs};

    use serde_json::json;
    use uuid::Uuid;

    use crate::langgraph_rs::cache::{
        base::{Cache, CacheKey, CacheSetOptions},
        memory::InMemoryCache,
        postgres::PostgresCache,
        sqlite::SqliteCache,
    };

    fn key(namespace: &[&str], key: &str) -> CacheKey {
        CacheKey::new(
            namespace
                .iter()
                .map(|segment| (*segment).to_owned())
                .collect(),
            key.to_owned(),
        )
    }

    fn run_conformance_suite<C: Cache>(cache: &C) {
        case_set_get_roundtrip(cache);
        case_update_preserves_created_at(cache);
        case_delete_semantics(cache);
        case_namespace_clear(cache);
        case_ttl_and_prune(cache);
    }

    fn case_set_get_roundtrip<C: Cache>(cache: &C) {
        let k = key(&["suite", "roundtrip"], "k");
        cache
            .set(
                &k,
                json!({"v": 1}),
                CacheSetOptions::new().with_ttl_millis(10_000),
            )
            .unwrap();
        let got = cache.get(&k).unwrap().unwrap();
        assert_eq!(got.value, json!({"v": 1}));
    }

    fn case_update_preserves_created_at<C: Cache>(cache: &C) {
        let k = key(&["suite", "update"], "k");
        let first = cache
            .set(
                &k,
                json!("a"),
                CacheSetOptions::new().with_ttl_millis(10_000),
            )
            .unwrap();
        let second = cache
            .set(
                &k,
                json!("b"),
                CacheSetOptions::new().with_ttl_millis(10_000),
            )
            .unwrap();

        assert_eq!(first.created_at_millis, second.created_at_millis);
        assert_eq!(second.value, json!("b"));
    }

    fn case_delete_semantics<C: Cache>(cache: &C) {
        let k = key(&["suite", "delete"], "k");
        cache
            .set(&k, json!(1), CacheSetOptions::new().with_ttl_millis(10_000))
            .unwrap();
        assert!(cache.delete(&k).unwrap());
        assert!(!cache.delete(&k).unwrap());
    }

    fn case_namespace_clear<C: Cache>(cache: &C) {
        let a1 = key(&["suite", "ns", "a"], "a1");
        let a2 = key(&["suite", "ns", "a"], "a2");
        let b1 = key(&["suite", "ns", "b"], "b1");
        cache
            .set(
                &a1,
                json!(1),
                CacheSetOptions::new().with_ttl_millis(10_000),
            )
            .unwrap();
        cache
            .set(
                &a2,
                json!(2),
                CacheSetOptions::new().with_ttl_millis(10_000),
            )
            .unwrap();
        cache
            .set(
                &b1,
                json!(3),
                CacheSetOptions::new().with_ttl_millis(10_000),
            )
            .unwrap();

        let cleared = cache
            .clear_namespace(&vec!["suite".to_owned(), "ns".to_owned(), "a".to_owned()])
            .unwrap();
        assert_eq!(cleared, 2);
        assert!(cache.get(&a1).unwrap().is_none());
        assert!(cache.get(&a2).unwrap().is_none());
        assert!(cache.get(&b1).unwrap().is_some());
    }

    fn case_ttl_and_prune<C: Cache>(cache: &C) {
        let k_read = key(&["suite", "ttl"], "k_read");
        let k_prune = key(&["suite", "ttl"], "k_prune");
        cache
            .set(
                &k_read,
                json!("x"),
                CacheSetOptions::new().with_ttl_millis(0),
            )
            .unwrap();
        cache
            .set(
                &k_prune,
                json!("y"),
                CacheSetOptions::new().with_ttl_millis(0),
            )
            .unwrap();

        // Backends may or may not eagerly delete expired entries on read.
        assert!(cache.get(&k_read).unwrap().is_none());
        let removed = cache.prune_expired().unwrap();
        assert!(removed >= 1);
        assert!(cache.get(&k_prune).unwrap().is_none());
    }

    #[test]
    fn memory_cache_passes_conformance_suite() {
        let cache = InMemoryCache::new();
        run_conformance_suite(&cache);
    }

    #[test]
    fn sqlite_cache_passes_conformance_suite() {
        let path = temp_dir().join(format!("langgraph_rs_cache_sqlite_{}.db", Uuid::new_v4()));
        let cache = SqliteCache::new(&path).unwrap();
        run_conformance_suite(&cache);
        drop(cache);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn postgres_cache_passes_conformance_suite_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let scope = format!("cache-conformance-{}", Uuid::new_v4().simple());
        let cache = PostgresCache::new(connection_string).unwrap();

        struct ScopedPostgresCache {
            inner: PostgresCache,
            scope: String,
        }

        impl Cache for ScopedPostgresCache {
            fn get(
                &self,
                key: &crate::langgraph_rs::cache::base::CacheKey,
            ) -> Result<
                Option<crate::langgraph_rs::cache::base::CacheItem>,
                crate::langgraph_rs::cache::base::CacheError,
            > {
                let scoped_key = crate::langgraph_rs::cache::base::CacheKey::new(
                    {
                        let mut ns = vec![self.scope.clone()];
                        ns.extend(key.namespace.clone());
                        ns
                    },
                    key.key.clone(),
                );
                self.inner.get(&scoped_key)
            }

            fn set(
                &self,
                key: &crate::langgraph_rs::cache::base::CacheKey,
                value: serde_json::Value,
                options: crate::langgraph_rs::cache::base::CacheSetOptions,
            ) -> Result<
                crate::langgraph_rs::cache::base::CacheItem,
                crate::langgraph_rs::cache::base::CacheError,
            > {
                let scoped_key = crate::langgraph_rs::cache::base::CacheKey::new(
                    {
                        let mut ns = vec![self.scope.clone()];
                        ns.extend(key.namespace.clone());
                        ns
                    },
                    key.key.clone(),
                );
                let mut item = self.inner.set(&scoped_key, value, options)?;
                if item.cache_key.namespace.first() == Some(&self.scope) {
                    item.cache_key.namespace.remove(0);
                }
                Ok(item)
            }

            fn delete(
                &self,
                key: &crate::langgraph_rs::cache::base::CacheKey,
            ) -> Result<bool, crate::langgraph_rs::cache::base::CacheError> {
                let scoped_key = crate::langgraph_rs::cache::base::CacheKey::new(
                    {
                        let mut ns = vec![self.scope.clone()];
                        ns.extend(key.namespace.clone());
                        ns
                    },
                    key.key.clone(),
                );
                self.inner.delete(&scoped_key)
            }

            fn clear_namespace(
                &self,
                namespace: &crate::langgraph_rs::cache::base::CacheNamespace,
            ) -> Result<usize, crate::langgraph_rs::cache::base::CacheError> {
                let mut scoped_ns = vec![self.scope.clone()];
                scoped_ns.extend(namespace.clone());
                self.inner.clear_namespace(&scoped_ns)
            }

            fn clear_all(&self) -> Result<usize, crate::langgraph_rs::cache::base::CacheError> {
                let mut total = 0usize;
                loop {
                    let removed = self.inner.clear_namespace(&vec![self.scope.clone()])?;
                    if removed == 0 {
                        break;
                    }
                    total = total.saturating_add(removed);
                }
                Ok(total)
            }

            fn prune_expired(&self) -> Result<usize, crate::langgraph_rs::cache::base::CacheError> {
                self.inner.prune_expired()
            }
        }

        let scoped = ScopedPostgresCache {
            inner: cache,
            scope,
        };
        run_conformance_suite(&scoped);
    }
}
