#![allow(dead_code)]

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use qdrant_client::{Payload, Qdrant, config::CompressionEncoding};
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, Distance, FieldType,
    Filter, NamedVectors, PointStruct, PrefetchQueryBuilder, QueryPointsBuilder,
    SparseVectorParamsBuilder, UpsertPointsBuilder, VectorParamsBuilder, Vector,
    with_payload_selector,
};
use qdrant_client::qdrant::vectors_config::Config as VectorsConfigInner;
use qdrant_client::qdrant::{VectorParams, VectorParamsMap, VectorsConfig};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::sparse;

const DEFAULT_QDRANT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_QDRANT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_JINA_BASE_URL: &str = "https://api.jina.ai/v1";
const DEFAULT_JINA_EMBEDDING_MODEL: &str = "jina-embeddings-v5-text-small";
const DEFAULT_JINA_EMBEDDING_TYPE: &str = "float";
const DEFAULT_JINA_TIMEOUT_SECS: u64 = 30;

#[derive(Clone, Debug)]
pub struct VectorDatabaseConfig {
    pub qdrant: QdrantCloudConfig,
    pub jina: JinaConfig,
}

impl VectorDatabaseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            qdrant: QdrantCloudConfig::from_env()?,
            jina: JinaConfig::from_env()?,
        })
    }
}

#[derive(Clone, Debug)]
pub struct QdrantCloudConfig {
    pub url: String,
    pub api_key: String,
    pub collection: Option<String>,
    pub timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub use_gzip_compression: bool,
    pub skip_compatibility_check: bool,
}

impl QdrantCloudConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            url: required_env("QDRANT_URL")?,
            api_key: required_env("QDRANT_API_KEY")?,
            collection: optional_env("QDRANT_COLLECTION"),
            timeout_secs: DEFAULT_QDRANT_TIMEOUT_SECS,
            connect_timeout_secs: DEFAULT_QDRANT_CONNECT_TIMEOUT_SECS,
            use_gzip_compression: false,
            skip_compatibility_check: false,
        })
    }
}

#[derive(Clone, Debug)]
pub struct JinaConfig {
    pub api_key: String,
    pub base_url: String,
    pub embedding_model: String,
    pub embedding_dimensions: Option<u32>,
    pub embedding_type: String,
    pub normalized: bool,
    pub reranker_model: Option<String>,
    pub timeout_secs: u64,
}

impl JinaConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            api_key: required_env("JINA_API_KEY")?,
            base_url: DEFAULT_JINA_BASE_URL.to_owned(),
            embedding_model: optional_env("JINA_EMBEDDING_MODEL")
                .unwrap_or_else(|| DEFAULT_JINA_EMBEDDING_MODEL.to_owned()),
            embedding_dimensions: optional_env_parse("JINA_EMBEDDING_DIMENSIONS")?,
            embedding_type: DEFAULT_JINA_EMBEDDING_TYPE.to_owned(),
            normalized: true,
            reranker_model: optional_env("JINA_RERANKER_MODEL"),
            timeout_secs: DEFAULT_JINA_TIMEOUT_SECS,
        })
    }
}

#[derive(Clone)]
pub struct VectorDatabaseClient {
    qdrant: Qdrant,
    jina_http: HttpClient,
    config: VectorDatabaseConfig,
}

impl VectorDatabaseClient {
    pub fn new(config: VectorDatabaseConfig) -> Result<Self> {
        let mut qdrant_builder = Qdrant::from_url(&config.qdrant.url)
            .api_key(Some(config.qdrant.api_key.clone()))
            .timeout(Duration::from_secs(config.qdrant.timeout_secs))
            .connect_timeout(Duration::from_secs(config.qdrant.connect_timeout_secs));

        if config.qdrant.use_gzip_compression {
            qdrant_builder = qdrant_builder.compression(Some(CompressionEncoding::Gzip));
        }

        if config.qdrant.skip_compatibility_check {
            qdrant_builder = qdrant_builder.skip_compatibility_check();
        }

        let qdrant = qdrant_builder
            .build()
            .context("Failed to create Qdrant Cloud client")?;

        let jina_http = HttpClient::builder()
            .timeout(Duration::from_secs(config.jina.timeout_secs))
            .build()
            .context("Failed to create Jina HTTP client")?;

        Ok(Self {
            qdrant,
            jina_http,
            config,
        })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(VectorDatabaseConfig::from_env()?)
    }

    pub fn qdrant(&self) -> &Qdrant {
        &self.qdrant
    }

    pub fn config(&self) -> &VectorDatabaseConfig {
        &self.config
    }

    pub async fn qdrant_health_check(&self) -> Result<()> {
        self.qdrant
            .health_check()
            .await
            .context("Qdrant Cloud health check failed")?;
        Ok(())
    }

    pub async fn list_collections(&self) -> Result<Vec<String>> {
        let response = self
            .qdrant
            .list_collections()
            .await
            .context("Failed to list Qdrant collections")?;

        Ok(response
            .collections
            .into_iter()
            .map(|collection| collection.name)
            .collect())
    }

    pub async fn embed_text(&self, input: impl Into<String>) -> Result<Vec<f32>> {
        let mut embeddings = self.embed_texts([input.into()]).await?;
        embeddings
            .pop()
            .ok_or_else(|| anyhow!("Jina returned no embedding for the requested input"))
    }

    pub async fn embed_texts<I, S>(&self, inputs: I) -> Result<Vec<Vec<f32>>>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let payload = JinaEmbeddingsRequest {
            model: self.config.jina.embedding_model.clone(),
            input: inputs.into_iter().map(Into::into).collect(),
            dimensions: self.config.jina.embedding_dimensions,
            embedding_type: Some(self.config.jina.embedding_type.clone()),
            normalized: Some(self.config.jina.normalized),
        };

        let response = self
            .jina_http
            .post(format!("{}/embeddings", self.config.jina.base_url))
            .bearer_auth(&self.config.jina.api_key)
            .json(&payload)
            .send()
            .await
            .context("Failed to call Jina embeddings API")?
            .error_for_status()
            .context("Jina embeddings API returned an error status")?
            .json::<JinaEmbeddingsResponse>()
            .await
            .context("Failed to deserialize Jina embeddings response")?;

        Ok(response
            .data
            .into_iter()
            .map(|item| item.embedding)
            .collect())
    }

    fn memories_collection_name() -> String {
        std::env::var("QDRANT_MEMORIES_COLLECTION")
            .unwrap_or_else(|_| "tradstry_memories".to_string())
    }

    /// Creates the `tradstry_hybrid` Qdrant collection with named dense+sparse vectors and
    /// payload indexes. Safe to call multiple times — skips creation if the collection already
    /// exists but still ensures indexes.
    pub async fn ensure_hybrid_collection(&self) -> Result<()> {
        const COLLECTION: &str = "tradstry_hybrid";

        let exists = self
            .qdrant
            .collection_exists(COLLECTION)
            .await
            .context("Failed to check if hybrid collection exists")?;

        if !exists {
            // Build named dense vector config
            let dense_params: VectorParams = VectorParamsBuilder::new(
                self.config
                    .jina
                    .embedding_dimensions
                    .unwrap_or(1024) as u64,
                Distance::Cosine,
            )
            .build();

            let mut params_map = HashMap::new();
            params_map.insert("dense".to_string(), dense_params);

            let vectors_config = VectorsConfig {
                config: Some(VectorsConfigInner::ParamsMap(VectorParamsMap {
                    map: params_map,
                })),
            };

            // Build named sparse vector config
            let sparse_params = SparseVectorParamsBuilder::default().build();
            let sparse_config = qdrant_client::qdrant::SparseVectorConfig {
                map: {
                    let mut m = HashMap::new();
                    m.insert("sparse".to_string(), sparse_params);
                    m
                },
            };

            self.qdrant
                .create_collection(
                    CreateCollectionBuilder::new(COLLECTION)
                        .vectors_config(vectors_config)
                        .sparse_vectors_config(sparse_config),
                )
                .await
                .context("Failed to create tradstry_hybrid collection")?;
        }

        // Ensure payload indexes (idempotent)
        for field in ["user_id", "account_id", "source_type", "created_at"] {
            self.qdrant
                .create_field_index(
                    CreateFieldIndexCollectionBuilder::new(COLLECTION, field, FieldType::Keyword),
                )
                .await
                .with_context(|| format!("Failed to create field index on {field}"))?;
        }

        Ok(())
    }

    /// Upserts a single document into `tradstry_hybrid` with both dense and sparse vectors.
    pub async fn upsert_hybrid(
        &self,
        point_id: &str,
        text: &str,
        user_id: &str,
        account_id: &str,
        source_type: &str,
        source_id: &str,
        created_at: &str,
    ) -> Result<()> {
        let dense_vec = self.embed_text(text).await?;
        let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(text);

        let named_vectors = NamedVectors::default()
            .add_vector("dense", Vector::new_dense(dense_vec))
            .add_vector("sparse", Vector::new_sparse(sparse_indices, sparse_values));

        let payload: Payload = Payload::try_from(json!({
            "user_id": user_id,
            "account_id": account_id,
            "source_type": source_type,
            "source_id": source_id,
            "created_at": created_at,
            "text": text,
        }))
        .context("Failed to build upsert payload")?;

        let point = PointStruct::new(point_id, named_vectors, payload);

        self.qdrant
            .upsert_points(
                UpsertPointsBuilder::new("tradstry_hybrid", vec![point]).wait(true),
            )
            .await
            .context("Failed to upsert hybrid point")?;

        Ok(())
    }

    /// Performs a hybrid (dense + sparse via RRF) search, then reranks with Jina.
    pub async fn hybrid_search(
        &self,
        query_text: &str,
        user_id: &str,
        account_id: &str,
        date_from: Option<&str>,
        date_to: Option<&str>,
        top_k: u64,
    ) -> Result<Vec<HybridSearchResult>> {
        // 1. Embed query
        let dense_vec: Vec<f32> = self.embed_text(query_text).await?;
        let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(query_text);

        // 2. Build filter conditions
        let mut conditions: Vec<Condition> = vec![
            Condition::matches("user_id", user_id.to_string()),
            Condition::matches("account_id", account_id.to_string()),
        ];
        if let Some(from) = date_from {
            conditions.push(Condition::matches("created_at", from.to_string()));
        }
        if let Some(to) = date_to {
            conditions.push(Condition::matches("created_at", to.to_string()));
        }
        let filter = Filter::must(conditions);

        // 3. Prefetch amount — fetch more candidates before reranking
        let prefetch_limit = (top_k * 4).max(20);

        // Build sparse prefetch: vec of (index, value) tuples
        let sparse_tuples: Vec<(u32, f32)> = sparse_indices
            .into_iter()
            .zip(sparse_values)
            .collect();

        let dense_prefetch = PrefetchQueryBuilder::default()
            .query(dense_vec)
            .using("dense")
            .filter(filter.clone())
            .limit(prefetch_limit);

        let sparse_prefetch = PrefetchQueryBuilder::default()
            .query(sparse_tuples)
            .using("sparse")
            .filter(filter)
            .limit(prefetch_limit);

        // 4. Run RRF fusion query
        let response = self
            .qdrant
            .query(
                QueryPointsBuilder::new("tradstry_hybrid")
                    .add_prefetch(dense_prefetch)
                    .add_prefetch(sparse_prefetch)
                    .query(qdrant_client::qdrant::Fusion::Rrf)
                    .limit(prefetch_limit)
                    .with_payload(with_payload_selector::SelectorOptions::Enable(true)),
            )
            .await
            .context("Hybrid search query failed")?;

        // 5. Extract text payloads from scored points
        let scored_points = response.result;
        if scored_points.is_empty() {
            return Ok(vec![]);
        }

        struct PointMeta {
            source_id: String,
            source_type: String,
            text: String,
        }

        let metas: Vec<PointMeta> = scored_points
            .iter()
            .map(|pt| {
                let get_str = |key: &str| -> String {
                    pt.payload
                        .get(key)
                        .and_then(|v| {
                            if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) =
                                &v.kind
                            {
                                Some(s.clone())
                            } else {
                                None
                            }
                        })
                        .unwrap_or_default()
                };
                PointMeta {
                    source_id: get_str("source_id"),
                    source_type: get_str("source_type"),
                    text: get_str("text"),
                }
            })
            .collect();

        let texts: Vec<String> = metas.iter().map(|m| m.text.clone()).collect();

        // 6. Rerank with Jina
        let rerank_results = self
            .rerank(query_text, texts.clone(), Some(top_k as u32))
            .await
            .context("Hybrid search reranking failed")?;

        // 7. Map back to HybridSearchResult using reranker indices
        let results: Vec<HybridSearchResult> = rerank_results
            .into_iter()
            .filter_map(|r| {
                metas.get(r.index).map(|meta| HybridSearchResult {
                    source_id: meta.source_id.clone(),
                    source_type: meta.source_type.clone(),
                    text: meta.text.clone(),
                    score: r.relevance_score as f64,
                })
            })
            .collect();

        Ok(results)
    }

    /// Creates the `tradstry_memories` Qdrant collection with named dense+sparse vectors and
    /// payload indexes. Safe to call multiple times — skips creation if the collection already
    /// exists but still ensures indexes.
    pub async fn ensure_memories_collection(&self) -> Result<()> {
        let collection = Self::memories_collection_name();

        let exists = self
            .qdrant
            .collection_exists(&collection)
            .await
            .context("Failed to check if memories collection exists")?;

        if !exists {
            // Build named dense vector config
            let dense_params: VectorParams = VectorParamsBuilder::new(
                self.config
                    .jina
                    .embedding_dimensions
                    .unwrap_or(1024) as u64,
                Distance::Cosine,
            )
            .build();

            let mut params_map = HashMap::new();
            params_map.insert("dense".to_string(), dense_params);

            let vectors_config = VectorsConfig {
                config: Some(VectorsConfigInner::ParamsMap(VectorParamsMap {
                    map: params_map,
                })),
            };

            // Build named sparse vector config
            let sparse_params = SparseVectorParamsBuilder::default().build();
            let sparse_config = qdrant_client::qdrant::SparseVectorConfig {
                map: {
                    let mut m = HashMap::new();
                    m.insert("sparse".to_string(), sparse_params);
                    m
                },
            };

            self.qdrant
                .create_collection(
                    CreateCollectionBuilder::new(&collection)
                        .vectors_config(vectors_config)
                        .sparse_vectors_config(sparse_config),
                )
                .await
                .context("Failed to create tradstry_memories collection")?;
        }

        // Ensure payload indexes (idempotent)
        for field in ["user_id", "memory_key"] {
            self.qdrant
                .create_field_index(
                    CreateFieldIndexCollectionBuilder::new(&collection, field, FieldType::Keyword),
                )
                .await
                .with_context(|| format!("Failed to create field index on {field}"))?;
        }

        Ok(())
    }

    /// Upserts a memory into `tradstry_memories` with both dense and sparse vectors.
    pub async fn upsert_memory(
        &self,
        user_id: &str,
        memory_key: &str,
        content: &str,
    ) -> Result<()> {
        let dense_vec = self.embed_text(content).await?;
        let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(content);

        let named_vectors = NamedVectors::default()
            .add_vector("dense", Vector::new_dense(dense_vec))
            .add_vector("sparse", Vector::new_sparse(sparse_indices, sparse_values));

        let payload: Payload = Payload::try_from(json!({
            "user_id": user_id,
            "memory_key": memory_key,
            "text": content,
        }))
        .context("Failed to build memory upsert payload")?;

        let point_id = uuid::Uuid::new_v4().to_string();
        let point = PointStruct::new(point_id, named_vectors, payload);

        self.qdrant
            .upsert_points(
                UpsertPointsBuilder::new(Self::memories_collection_name(), vec![point]).wait(true),
            )
            .await
            .context("Failed to upsert memory point")?;

        Ok(())
    }

    /// Searches `tradstry_memories` using hybrid (dense + sparse via RRF) search filtered by
    /// `user_id`. Returns the `text` payload from each result.
    pub async fn search_memories(
        &self,
        query_text: &str,
        user_id: &str,
        top_k: u64,
    ) -> Result<Vec<String>> {
        // 1. Embed query
        let dense_vec: Vec<f32> = self.embed_text(query_text).await?;
        let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(query_text);

        // 2. Build filter — only by user_id
        let filter = Filter::must(vec![Condition::matches("user_id", user_id.to_string())]);

        // 3. Prefetch amount — fetch more candidates before fusion
        let prefetch_limit = (top_k * 4).max(20);

        let sparse_tuples: Vec<(u32, f32)> = sparse_indices
            .into_iter()
            .zip(sparse_values)
            .collect();

        let dense_prefetch = PrefetchQueryBuilder::default()
            .query(dense_vec)
            .using("dense")
            .filter(filter.clone())
            .limit(prefetch_limit);

        let sparse_prefetch = PrefetchQueryBuilder::default()
            .query(sparse_tuples)
            .using("sparse")
            .filter(filter)
            .limit(prefetch_limit);

        // 4. Run RRF fusion query
        let response = self
            .qdrant
            .query(
                QueryPointsBuilder::new(Self::memories_collection_name())
                    .add_prefetch(dense_prefetch)
                    .add_prefetch(sparse_prefetch)
                    .query(qdrant_client::qdrant::Fusion::Rrf)
                    .limit(top_k)
                    .with_payload(with_payload_selector::SelectorOptions::Enable(true)),
            )
            .await
            .context("Memories search query failed")?;

        // 5. Extract text payloads
        let texts: Vec<String> = response
            .result
            .iter()
            .map(|pt| {
                pt.payload
                    .get("text")
                    .and_then(|v| {
                        if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                            Some(s.clone())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_default()
            })
            .collect();

        Ok(texts)
    }

    pub async fn rerank(
        &self,
        query: impl Into<String>,
        documents: Vec<String>,
        top_n: Option<u32>,
    ) -> Result<Vec<JinaRerankResult>> {
        let payload = JinaRerankRequest {
            model: self.config.jina.reranker_model.clone(),
            query: query.into(),
            documents,
            top_n,
        };

        let response = self
            .jina_http
            .post(format!("{}/rerank", self.config.jina.base_url))
            .bearer_auth(&self.config.jina.api_key)
            .json(&payload)
            .send()
            .await
            .context("Failed to call Jina rerank API")?
            .error_for_status()
            .context("Jina rerank API returned an error status")?
            .json::<JinaRerankResponse>()
            .await
            .context("Failed to deserialize Jina rerank response")?;

        Ok(response.results)
    }
}

/// Result from a hybrid (dense + sparse) search with Jina reranking.
#[derive(Clone, Debug, Serialize)]
pub struct HybridSearchResult {
    pub source_id: String,
    pub source_type: String,
    pub text: String,
    pub score: f64,
}

#[derive(Debug, Serialize)]
struct JinaEmbeddingsRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    embedding_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    normalized: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JinaEmbeddingsResponse {
    data: Vec<JinaEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct JinaEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct JinaRerankRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    query: String,
    documents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_n: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct JinaRerankResponse {
    results: Vec<JinaRerankResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct JinaRerankResult {
    pub index: usize,
    pub relevance_score: f32,
}

fn required_env(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("{name} environment variable not set"))
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn optional_env_parse<T>(name: &str) -> Result<Option<T>>
where
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    optional_env(name)
        .map(|value| {
            value
                .parse::<T>()
                .map_err(|error| anyhow!("{name} is invalid: {error}"))
        })
        .transpose()
}

fn optional_env_bool(name: &str) -> Result<Option<bool>> {
    optional_env(name)
        .map(|value| match value.to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" => Ok(false),
            _ => Err(anyhow!(
                "{name} must be one of true/false, 1/0, yes/no, on/off"
            )),
        })
        .transpose()
}
