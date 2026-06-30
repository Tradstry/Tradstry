use std::sync::Arc;

use langgraph::prelude::{
    EmbeddingVector, NamespacePath, PostgresStore, Store, StoreError, StoreItem, StoreListQuery,
    StoreScoredItem, StoreSearchQuery, StoreVectorQuery,
};
use log::{info, warn};
use serde_json::Value;

/// A wrapper around PostgresStore that delegates all calls to a dedicated OS thread
/// to avoid the "cannot start runtime from within a runtime" panic. The synchronous
/// `postgres` crate internally uses `block_on`, which panics inside tokio.
struct BlockingPostgresStore {
    inner: Arc<PostgresStore>,
}

impl BlockingPostgresStore {
    fn new(store: PostgresStore) -> Self {
        Self {
            inner: Arc::new(store),
        }
    }

    /// Run a closure on a fresh OS thread (not a tokio thread) to avoid runtime nesting.
    fn block<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&PostgresStore) -> R + Send + 'static,
        R: Send + 'static,
    {
        let inner = self.inner.clone();
        std::thread::scope(|s| {
            s.spawn(move || f(&inner))
                .join()
                .expect("blocking store thread panicked")
        })
    }
}

impl Store for BlockingPostgresStore {
    fn put(
        &self,
        namespace: &NamespacePath,
        key: &str,
        value: Value,
    ) -> Result<StoreItem, StoreError> {
        let namespace = namespace.clone();
        let key = key.to_string();
        self.block(move |store| store.put(&namespace, &key, value))
    }

    fn get(&self, namespace: &NamespacePath, key: &str) -> Result<Option<StoreItem>, StoreError> {
        let namespace = namespace.clone();
        let key = key.to_string();
        self.block(move |store| store.get(&namespace, &key))
    }

    fn delete(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        let namespace = namespace.clone();
        let key = key.to_string();
        self.block(move |store| store.delete(&namespace, &key))
    }

    fn list(&self, query: &StoreListQuery) -> Result<Vec<StoreItem>, StoreError> {
        let query = query.clone();
        self.block(move |store| store.list(&query))
    }

    fn search(&self, query: &StoreSearchQuery) -> Result<Vec<StoreItem>, StoreError> {
        let query = query.clone();
        self.block(move |store| store.search(&query))
    }

    fn put_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
        embedding: EmbeddingVector,
    ) -> Result<(), StoreError> {
        let namespace = namespace.clone();
        let key = key.to_string();
        self.block(move |store| store.put_embedding(&namespace, &key, embedding))
    }

    fn get_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
    ) -> Result<Option<EmbeddingVector>, StoreError> {
        let namespace = namespace.clone();
        let key = key.to_string();
        self.block(move |store| store.get_embedding(&namespace, &key))
    }

    fn delete_embedding(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        let namespace = namespace.clone();
        let key = key.to_string();
        self.block(move |store| store.delete_embedding(&namespace, &key))
    }

    fn vector_search(&self, query: &StoreVectorQuery) -> Result<Vec<StoreScoredItem>, StoreError> {
        let query = query.clone();
        self.block(move |store| store.vector_search(&query))
    }
}

/// Initialize the memory store: try Postgres, return None on failure.
pub async fn init_memory_store() -> Option<Arc<dyn Store>> {
    match std::env::var("POSTGRES_URL") {
        Ok(url) if !url.is_empty() => {
            // Route memory-store tables into the per-environment schema
            // (POSTGRES_DATABASE) via the libpq search_path connection option.
            let url = crate::service::db::config::url_with_search_path(&url).unwrap_or(url);
            let result = tokio::task::spawn_blocking(move || PostgresStore::new(&url)).await;

            match result {
                Ok(Ok(store)) => {
                    info!("Memory store: Postgres (wrapped for async compatibility)");
                    return Some(Arc::new(BlockingPostgresStore::new(store)));
                }
                Ok(Err(e)) => {
                    warn!(
                        "Failed to connect to Postgres for memory store: {e}. Memory store disabled."
                    );
                }
                Err(e) => {
                    warn!("Postgres memory store init panicked: {e}. Memory store disabled.");
                }
            }
        }
        _ => {
            info!("POSTGRES_URL not set, memory store disabled");
        }
    }

    None
}
