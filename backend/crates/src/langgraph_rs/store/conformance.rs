#[cfg(test)]
mod tests {
    use std::{env::temp_dir, fs};

    use serde_json::json;
    use uuid::Uuid;

    use crate::langgraph_rs::store::{
        base::{
            EmbeddingVector, NamespacePath, Store, StoreError, StoreItem, StoreListQuery,
            StoreScoredItem, StoreSearchQuery, StoreVectorQuery,
        },
        memory::InMemoryStore,
        postgres::PostgresStore,
        sqlite::SqliteStore,
    };

    fn namespace(parts: &[&str]) -> NamespacePath {
        parts.iter().map(|part| (*part).to_owned()).collect()
    }

    fn run_conformance_suite<S: Store>(store: &S) {
        case_put_get_roundtrip(store);
        case_put_updates_existing_value(store);
        case_list_namespace_filters(store);
        case_search_namespace_prefix(store);
        case_delete_semantics(store);
        case_embedding_roundtrip(store);
        case_vector_search(store);
    }

    fn case_put_get_roundtrip<S: Store>(store: &S) {
        let ns = namespace(&["suite", "put_get"]);
        let stored = store.put(&ns, "k1", json!({"v": 1})).unwrap();
        let loaded = store.get(&ns, "k1").unwrap().unwrap();

        assert_eq!(stored.namespace, ns);
        assert_eq!(loaded.value, json!({"v": 1}));
    }

    fn case_put_updates_existing_value<S: Store>(store: &S) {
        let ns = namespace(&["suite", "update"]);
        let first = store.put(&ns, "k", json!("first")).unwrap();
        let second = store.put(&ns, "k", json!("second")).unwrap();

        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.value, json!("second"));
    }

    fn case_list_namespace_filters<S: Store>(store: &S) {
        let ns_a = namespace(&["suite", "list", "a"]);
        let ns_b = namespace(&["suite", "list", "b"]);
        store.put(&ns_a, "a1", json!(1)).unwrap();
        store.put(&ns_b, "b1", json!(2)).unwrap();

        let exact = store
            .list(&StoreListQuery {
                namespace: Some(ns_a.clone()),
                namespace_prefix: None,
                limit: None,
            })
            .unwrap();
        assert_eq!(exact.len(), 1);
        assert_eq!(exact[0].namespace, ns_a);

        let prefix = store
            .list(&StoreListQuery {
                namespace: None,
                namespace_prefix: Some(namespace(&["suite", "list"])),
                limit: None,
            })
            .unwrap();
        assert!(prefix.len() >= 2);
    }

    fn case_search_namespace_prefix<S: Store>(store: &S) {
        let ns_a = namespace(&["suite", "search", "a"]);
        let ns_b = namespace(&["suite", "search", "b"]);
        store
            .put(&ns_a, "profile", json!({"city":"Lagos"}))
            .unwrap();
        store
            .put(&ns_b, "profile", json!({"city":"Nairobi"}))
            .unwrap();

        let matches = store
            .search(
                &StoreSearchQuery::new("lag")
                    .with_namespace_prefix(namespace(&["suite", "search", "a"])),
            )
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].namespace, ns_a);
    }

    fn case_delete_semantics<S: Store>(store: &S) {
        let ns = namespace(&["suite", "delete"]);
        store.put(&ns, "k", json!(1)).unwrap();
        assert!(store.delete(&ns, "k").unwrap());
        assert!(!store.delete(&ns, "k").unwrap());
        assert!(store.get(&ns, "k").unwrap().is_none());
    }

    fn case_embedding_roundtrip<S: Store>(store: &S) {
        let ns = namespace(&["suite", "embedding"]);
        store.put(&ns, "item", json!({"kind":"vector"})).unwrap();
        let embedding = vec![0.2_f32, 0.4, 0.6];
        store.put_embedding(&ns, "item", embedding.clone()).unwrap();

        let loaded = store.get_embedding(&ns, "item").unwrap().unwrap();
        assert_eq!(loaded, embedding);
        assert!(store.delete_embedding(&ns, "item").unwrap());
        assert!(store.get_embedding(&ns, "item").unwrap().is_none());
    }

    fn case_vector_search<S: Store>(store: &S) {
        let ns_a = namespace(&["suite", "vector", "a"]);
        let ns_b = namespace(&["suite", "vector", "b"]);

        store.put(&ns_a, "a1", json!({"label":"alpha"})).unwrap();
        store.put(&ns_b, "b1", json!({"label":"beta"})).unwrap();
        store.put_embedding(&ns_a, "a1", vec![1.0, 0.0]).unwrap();
        store.put_embedding(&ns_b, "b1", vec![0.0, 1.0]).unwrap();

        let matches = store
            .vector_search(
                &StoreVectorQuery::new(vec![1.0, 0.0])
                    .with_namespace_prefix(namespace(&["suite", "vector"]))
                    .with_limit(1),
            )
            .unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].item.key, "a1");
    }

    #[test]
    fn in_memory_store_passes_conformance_suite() {
        let store = InMemoryStore::new();
        run_conformance_suite(&store);
    }

    #[test]
    fn sqlite_store_passes_conformance_suite() {
        let path = temp_dir().join(format!("langgraph_rs_store_sqlite_{}.db", Uuid::new_v4()));
        let store = SqliteStore::new(&path).unwrap();
        run_conformance_suite(&store);
        drop(store);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn postgres_store_passes_conformance_suite_when_env_is_set() {
        let Ok(connection_string) = std::env::var("LANGGRAPH_RS_TEST_POSTGRES_URL") else {
            return;
        };
        let scope = format!("store-conformance-{}", Uuid::new_v4().simple());
        let store = PostgresStore::new(connection_string).unwrap();

        struct ScopedPostgresStore {
            inner: PostgresStore,
            scope: String,
        }

        impl Store for ScopedPostgresStore {
            fn put(
                &self,
                namespace: &NamespacePath,
                key: &str,
                value: serde_json::Value,
            ) -> Result<StoreItem, StoreError> {
                let mut scoped = vec![self.scope.clone()];
                scoped.extend(namespace.clone());
                self.inner.put(&scoped, key, value)
            }

            fn get(
                &self,
                namespace: &NamespacePath,
                key: &str,
            ) -> Result<Option<StoreItem>, StoreError> {
                let mut scoped = vec![self.scope.clone()];
                scoped.extend(namespace.clone());
                self.inner.get(&scoped, key)
            }

            fn delete(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
                let mut scoped = vec![self.scope.clone()];
                scoped.extend(namespace.clone());
                self.inner.delete(&scoped, key)
            }

            fn list(&self, query: &StoreListQuery) -> Result<Vec<StoreItem>, StoreError> {
                let scoped_namespace = query.namespace.as_ref().map(|ns| {
                    let mut scoped = vec![self.scope.clone()];
                    scoped.extend(ns.clone());
                    scoped
                });
                let scoped_prefix = query.namespace_prefix.as_ref().map(|ns| {
                    let mut scoped = vec![self.scope.clone()];
                    scoped.extend(ns.clone());
                    scoped
                });
                let scoped_query = StoreListQuery {
                    namespace: scoped_namespace,
                    namespace_prefix: scoped_prefix,
                    limit: query.limit,
                };

                let mut items = self.inner.list(&scoped_query)?;
                for item in &mut items {
                    if item.namespace.first() == Some(&self.scope) {
                        item.namespace.remove(0);
                    }
                }
                Ok(items)
            }

            fn search(&self, query: &StoreSearchQuery) -> Result<Vec<StoreItem>, StoreError> {
                let scoped_query = if let Some(prefix) = &query.namespace_prefix {
                    let mut scoped_prefix = vec![self.scope.clone()];
                    scoped_prefix.extend(prefix.clone());
                    StoreSearchQuery {
                        query: query.query.clone(),
                        namespace_prefix: Some(scoped_prefix),
                        limit: query.limit,
                    }
                } else {
                    StoreSearchQuery {
                        query: query.query.clone(),
                        namespace_prefix: Some(vec![self.scope.clone()]),
                        limit: query.limit,
                    }
                };

                let mut items = self.inner.search(&scoped_query)?;
                for item in &mut items {
                    if item.namespace.first() == Some(&self.scope) {
                        item.namespace.remove(0);
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
                let mut scoped = vec![self.scope.clone()];
                scoped.extend(namespace.clone());
                self.inner.put_embedding(&scoped, key, embedding)
            }

            fn get_embedding(
                &self,
                namespace: &NamespacePath,
                key: &str,
            ) -> Result<Option<EmbeddingVector>, StoreError> {
                let mut scoped = vec![self.scope.clone()];
                scoped.extend(namespace.clone());
                self.inner.get_embedding(&scoped, key)
            }

            fn delete_embedding(
                &self,
                namespace: &NamespacePath,
                key: &str,
            ) -> Result<bool, StoreError> {
                let mut scoped = vec![self.scope.clone()];
                scoped.extend(namespace.clone());
                self.inner.delete_embedding(&scoped, key)
            }

            fn vector_search(
                &self,
                query: &StoreVectorQuery,
            ) -> Result<Vec<StoreScoredItem>, StoreError> {
                let scoped_query = if let Some(prefix) = &query.namespace_prefix {
                    let mut scoped_prefix = vec![self.scope.clone()];
                    scoped_prefix.extend(prefix.clone());
                    StoreVectorQuery {
                        embedding: query.embedding.clone(),
                        namespace_prefix: Some(scoped_prefix),
                        limit: query.limit,
                        min_score: query.min_score,
                        metric: query.metric,
                    }
                } else {
                    StoreVectorQuery {
                        embedding: query.embedding.clone(),
                        namespace_prefix: Some(vec![self.scope.clone()]),
                        limit: query.limit,
                        min_score: query.min_score,
                        metric: query.metric,
                    }
                };

                let mut items = self.inner.vector_search(&scoped_query)?;
                for item in &mut items {
                    if item.item.namespace.first() == Some(&self.scope) {
                        item.item.namespace.remove(0);
                    }
                }
                Ok(items)
            }
        }

        let scoped = ScopedPostgresStore {
            inner: store,
            scope,
        };
        run_conformance_suite(&scoped);
    }
}
