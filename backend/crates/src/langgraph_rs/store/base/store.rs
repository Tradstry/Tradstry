use async_trait::async_trait;
use serde_json::Value;

use super::{
    EmbeddingVector, NamespacePath, StoreError, StoreItem, StoreListQuery, StoreScoredItem,
    StoreSearchQuery, StoreVectorQuery,
};

#[async_trait]
pub trait Store: Send + Sync {
    fn put(
        &self,
        _namespace: &NamespacePath,
        _key: &str,
        _value: Value,
    ) -> Result<StoreItem, StoreError> {
        Err(StoreError::not_implemented("put"))
    }

    fn get(&self, _namespace: &NamespacePath, _key: &str) -> Result<Option<StoreItem>, StoreError> {
        Err(StoreError::not_implemented("get"))
    }

    fn delete(&self, _namespace: &NamespacePath, _key: &str) -> Result<bool, StoreError> {
        Err(StoreError::not_implemented("delete"))
    }

    fn list(&self, _query: &StoreListQuery) -> Result<Vec<StoreItem>, StoreError> {
        Err(StoreError::not_implemented("list"))
    }

    fn search(&self, _query: &StoreSearchQuery) -> Result<Vec<StoreItem>, StoreError> {
        Err(StoreError::not_implemented("search"))
    }

    fn put_embedding(
        &self,
        _namespace: &NamespacePath,
        _key: &str,
        _embedding: EmbeddingVector,
    ) -> Result<(), StoreError> {
        Err(StoreError::unsupported_capability("vector_embeddings"))
    }

    fn get_embedding(
        &self,
        _namespace: &NamespacePath,
        _key: &str,
    ) -> Result<Option<EmbeddingVector>, StoreError> {
        Err(StoreError::unsupported_capability("vector_embeddings"))
    }

    fn delete_embedding(&self, _namespace: &NamespacePath, _key: &str) -> Result<bool, StoreError> {
        Err(StoreError::unsupported_capability("vector_embeddings"))
    }

    fn vector_search(&self, _query: &StoreVectorQuery) -> Result<Vec<StoreScoredItem>, StoreError> {
        Err(StoreError::unsupported_capability("vector_search"))
    }

    fn put_with_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
        value: Value,
        embedding: EmbeddingVector,
    ) -> Result<StoreItem, StoreError> {
        let item = self.put(namespace, key, value)?;
        self.put_embedding(namespace, key, embedding)?;
        Ok(item)
    }

    async fn aput(
        &self,
        namespace: &NamespacePath,
        key: &str,
        value: Value,
    ) -> Result<StoreItem, StoreError> {
        self.put(namespace, key, value)
    }

    async fn aget(
        &self,
        namespace: &NamespacePath,
        key: &str,
    ) -> Result<Option<StoreItem>, StoreError> {
        self.get(namespace, key)
    }

    async fn adelete(&self, namespace: &NamespacePath, key: &str) -> Result<bool, StoreError> {
        self.delete(namespace, key)
    }

    async fn alist(&self, query: &StoreListQuery) -> Result<Vec<StoreItem>, StoreError> {
        self.list(query)
    }

    async fn asearch(&self, query: &StoreSearchQuery) -> Result<Vec<StoreItem>, StoreError> {
        self.search(query)
    }

    async fn aput_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
        embedding: EmbeddingVector,
    ) -> Result<(), StoreError> {
        self.put_embedding(namespace, key, embedding)
    }

    async fn aget_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
    ) -> Result<Option<EmbeddingVector>, StoreError> {
        self.get_embedding(namespace, key)
    }

    async fn adelete_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
    ) -> Result<bool, StoreError> {
        self.delete_embedding(namespace, key)
    }

    async fn avector_search(
        &self,
        query: &StoreVectorQuery,
    ) -> Result<Vec<StoreScoredItem>, StoreError> {
        self.vector_search(query)
    }

    async fn aput_with_embedding(
        &self,
        namespace: &NamespacePath,
        key: &str,
        value: Value,
        embedding: EmbeddingVector,
    ) -> Result<StoreItem, StoreError> {
        self.put_with_embedding(namespace, key, value, embedding)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::Store;

    struct NoopStore;

    impl Store for NoopStore {}

    #[test]
    fn default_trait_methods_surface_not_implemented() {
        let store = NoopStore;
        let error = store
            .list(&crate::langgraph_rs::store::base::StoreListQuery::default())
            .unwrap_err();
        assert!(format!("{error}").contains("not implemented"));
    }

    #[test]
    fn vector_methods_surface_unsupported_by_default() {
        let store = NoopStore;
        let ns = vec!["a".to_owned()];
        let error = store.put_embedding(&ns, "k", vec![1.0]).unwrap_err();
        assert!(format!("{error}").contains("not supported"));
    }

    struct InMemoryLikeStore;

    impl Store for InMemoryLikeStore {
        fn put(
            &self,
            namespace: &crate::langgraph_rs::store::base::NamespacePath,
            key: &str,
            value: serde_json::Value,
        ) -> Result<
            crate::langgraph_rs::store::base::StoreItem,
            crate::langgraph_rs::store::base::StoreError,
        > {
            Ok(crate::langgraph_rs::store::base::StoreItem::new(
                namespace.clone(),
                key,
                value,
            ))
        }

        fn put_embedding(
            &self,
            _namespace: &crate::langgraph_rs::store::base::NamespacePath,
            _key: &str,
            _embedding: crate::langgraph_rs::store::base::EmbeddingVector,
        ) -> Result<(), crate::langgraph_rs::store::base::StoreError> {
            Ok(())
        }
    }

    #[test]
    fn put_with_embedding_composes_put_and_embedding_storage() {
        let store = InMemoryLikeStore;
        let ns = vec!["a".to_owned()];
        let item = store
            .put_with_embedding(&ns, "k", json!({"v": 1}), vec![0.1, 0.2])
            .unwrap();
        assert_eq!(item.key, "k");
    }
}
