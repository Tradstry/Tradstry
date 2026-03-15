use async_trait::async_trait;
use serde_json::Value;

use super::{CacheError, CacheItem, CacheKey, CacheNamespace, CacheSetOptions};

#[async_trait]
pub trait Cache: Send + Sync {
    fn get(&self, _key: &CacheKey) -> Result<Option<CacheItem>, CacheError> {
        Err(CacheError::not_implemented("get"))
    }

    fn set(
        &self,
        _key: &CacheKey,
        _value: Value,
        _options: CacheSetOptions,
    ) -> Result<CacheItem, CacheError> {
        Err(CacheError::not_implemented("set"))
    }

    fn delete(&self, _key: &CacheKey) -> Result<bool, CacheError> {
        Err(CacheError::not_implemented("delete"))
    }

    fn clear_namespace(&self, _namespace: &CacheNamespace) -> Result<usize, CacheError> {
        Err(CacheError::unsupported_capability("clear_namespace"))
    }

    fn clear_all(&self) -> Result<usize, CacheError> {
        Err(CacheError::unsupported_capability("clear_all"))
    }

    fn prune_expired(&self) -> Result<usize, CacheError> {
        Err(CacheError::unsupported_capability("prune_expired"))
    }

    async fn aget(&self, key: &CacheKey) -> Result<Option<CacheItem>, CacheError> {
        self.get(key)
    }

    async fn aset(
        &self,
        key: &CacheKey,
        value: Value,
        options: CacheSetOptions,
    ) -> Result<CacheItem, CacheError> {
        self.set(key, value, options)
    }

    async fn adelete(&self, key: &CacheKey) -> Result<bool, CacheError> {
        self.delete(key)
    }

    async fn aclear_namespace(&self, namespace: &CacheNamespace) -> Result<usize, CacheError> {
        self.clear_namespace(namespace)
    }

    async fn aclear_all(&self) -> Result<usize, CacheError> {
        self.clear_all()
    }

    async fn aprune_expired(&self) -> Result<usize, CacheError> {
        self.prune_expired()
    }
}

#[cfg(test)]
mod tests {
    use super::Cache;

    struct NoopCache;

    impl Cache for NoopCache {}

    #[test]
    fn default_methods_surface_not_implemented_or_unsupported() {
        let cache = NoopCache;
        let get_err = cache
            .get(&crate::langgraph_rs::cache::base::CacheKey::new(
                vec!["a".to_owned()],
                "k",
            ))
            .unwrap_err();
        let clear_err = cache.clear_all().unwrap_err();

        assert!(format!("{get_err}").contains("not implemented"));
        assert!(format!("{clear_err}").contains("not supported"));
    }
}
