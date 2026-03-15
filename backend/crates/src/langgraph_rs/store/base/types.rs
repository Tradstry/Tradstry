use std::{
    collections::BTreeMap,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub type NamespacePath = Vec<String>;
pub type StoreMetadata = BTreeMap<String, Value>;
pub type EmbeddingVector = Vec<f32>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreItem {
    pub namespace: NamespacePath,
    pub key: String,
    pub value: Value,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: StoreMetadata,
}

impl StoreItem {
    pub fn new(namespace: NamespacePath, key: impl Into<String>, value: Value) -> Self {
        let now = now_timestamp_string();
        Self {
            namespace,
            key: key.into(),
            value,
            created_at: now.clone(),
            updated_at: now,
            metadata: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StoreListQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace: Option<NamespacePath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_prefix: Option<NamespacePath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreSearchQuery {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_prefix: Option<NamespacePath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

impl StoreSearchQuery {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            namespace_prefix: None,
            limit: None,
        }
    }

    pub fn with_namespace_prefix(mut self, namespace_prefix: NamespacePath) -> Self {
        self.namespace_prefix = Some(namespace_prefix);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum VectorMetric {
    #[default]
    Cosine,
    DotProduct,
    Euclidean,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreVectorQuery {
    pub embedding: EmbeddingVector,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub namespace_prefix: Option<NamespacePath>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_score: Option<f32>,
    #[serde(default)]
    pub metric: VectorMetric,
}

impl StoreVectorQuery {
    pub fn new(embedding: EmbeddingVector) -> Self {
        Self {
            embedding,
            namespace_prefix: None,
            limit: None,
            min_score: None,
            metric: VectorMetric::Cosine,
        }
    }

    pub fn with_namespace_prefix(mut self, namespace_prefix: NamespacePath) -> Self {
        self.namespace_prefix = Some(namespace_prefix);
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    pub fn with_min_score(mut self, min_score: f32) -> Self {
        self.min_score = Some(min_score);
        self
    }

    pub fn with_metric(mut self, metric: VectorMetric) -> Self {
        self.metric = metric;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoreScoredItem {
    pub item: StoreItem,
    pub score: f32,
}

impl StoreScoredItem {
    pub fn new(item: StoreItem, score: f32) -> Self {
        Self { item, score }
    }
}

pub fn namespace_matches_prefix(namespace: &[String], prefix: &[String]) -> bool {
    namespace.len() >= prefix.len()
        && namespace
            .iter()
            .zip(prefix.iter())
            .all(|(value, expected)| value == expected)
}

pub fn now_timestamp_string() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}.{:09}Z", now.as_secs(), now.subsec_nanos())
}

pub fn vector_score(query: &[f32], candidate: &[f32], metric: VectorMetric) -> Option<f32> {
    if query.is_empty() || candidate.is_empty() || query.len() != candidate.len() {
        return None;
    }
    if query.iter().any(|v| !v.is_finite()) || candidate.iter().any(|v| !v.is_finite()) {
        return None;
    }

    match metric {
        VectorMetric::DotProduct => Some(
            query
                .iter()
                .zip(candidate.iter())
                .map(|(a, b)| a * b)
                .sum::<f32>(),
        ),
        VectorMetric::Cosine => {
            let dot = query
                .iter()
                .zip(candidate.iter())
                .map(|(a, b)| a * b)
                .sum::<f32>();
            let query_norm = query.iter().map(|v| v * v).sum::<f32>().sqrt();
            let candidate_norm = candidate.iter().map(|v| v * v).sum::<f32>().sqrt();
            let denom = query_norm * candidate_norm;
            if denom <= f32::EPSILON {
                None
            } else {
                Some(dot / denom)
            }
        }
        VectorMetric::Euclidean => {
            let distance = query
                .iter()
                .zip(candidate.iter())
                .map(|(a, b)| {
                    let diff = a - b;
                    diff * diff
                })
                .sum::<f32>()
                .sqrt();
            Some(1.0 / (1.0 + distance))
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{StoreItem, VectorMetric, namespace_matches_prefix, vector_score};

    #[test]
    fn item_initializes_with_timestamps() {
        let item = StoreItem::new(vec!["a".to_owned()], "k", json!({"x": 1}));

        assert_eq!(item.namespace, vec!["a".to_owned()]);
        assert_eq!(item.key, "k");
        assert!(!item.created_at.is_empty());
        assert!(!item.updated_at.is_empty());
    }

    #[test]
    fn prefix_matching_is_path_based() {
        assert!(namespace_matches_prefix(
            &["a".to_owned(), "b".to_owned()],
            &["a".to_owned()]
        ));
        assert!(!namespace_matches_prefix(
            &["a".to_owned()],
            &["a".to_owned(), "b".to_owned()]
        ));
        assert!(!namespace_matches_prefix(
            &["x".to_owned(), "b".to_owned()],
            &["a".to_owned()]
        ));
    }

    #[test]
    fn vector_score_handles_metrics_and_validation() {
        let q = vec![1.0_f32, 0.0];
        let same = vec![1.0_f32, 0.0];
        let orth = vec![0.0_f32, 1.0];

        let cosine_same = vector_score(&q, &same, VectorMetric::Cosine).unwrap();
        let cosine_orth = vector_score(&q, &orth, VectorMetric::Cosine).unwrap();
        let dot_same = vector_score(&q, &same, VectorMetric::DotProduct).unwrap();
        let euclid_same = vector_score(&q, &same, VectorMetric::Euclidean).unwrap();

        assert!(cosine_same > cosine_orth);
        assert_eq!(dot_same, 1.0);
        assert_eq!(euclid_same, 1.0);
        assert!(vector_score(&q, &[1.0], VectorMetric::Cosine).is_none());
    }
}
