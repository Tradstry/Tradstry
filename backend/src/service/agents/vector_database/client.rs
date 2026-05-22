#![allow(dead_code)]

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use qdrant_client::qdrant::vectors_config::Config as VectorsConfigInner;
use qdrant_client::qdrant::{
    Condition, CreateCollectionBuilder, CreateFieldIndexCollectionBuilder, Distance, FieldType,
    Filter, GetPointsBuilder, NamedVectors, PointStruct, PrefetchQueryBuilder, QueryPointsBuilder,
    SparseVectorParamsBuilder, UpsertPointsBuilder, Vector, VectorParamsBuilder,
    with_payload_selector,
};
use qdrant_client::qdrant::{VectorParams, VectorParamsMap, VectorsConfig};
use qdrant_client::{Payload, Qdrant, config::CompressionEncoding};
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::Mutex;

use super::sparse;

const DEFAULT_QDRANT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_QDRANT_CONNECT_TIMEOUT_SECS: u64 = 10;
const DEFAULT_VOYAGE_BASE_URL: &str = "https://api.voyageai.com/v1";
const DEFAULT_VOYAGE_EMBEDDING_MODEL: &str = "voyage-3.5";
const DEFAULT_VOYAGE_OUTPUT_DIMENSION: u32 = 2048;
const DEFAULT_VOYAGE_RERANKER_MODEL: &str = "rerank-2.5";
const DEFAULT_VOYAGE_TIMEOUT_SECS: u64 = 30;
// Voyage no-payment tier: 3 RPM / 10K TPM. Override via env once a payment
// method raises the limits (Tier 1 is 2000 RPM / 8M TPM for voyage-3.5).
const DEFAULT_VOYAGE_RPM: u32 = 3;
const DEFAULT_VOYAGE_TPM: u32 = 10_000;

#[derive(Clone, Debug)]
pub struct VectorDatabaseConfig {
    pub qdrant: QdrantCloudConfig,
    pub voyage: VoyageConfig,
}

impl VectorDatabaseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            qdrant: QdrantCloudConfig::from_env()?,
            voyage: VoyageConfig::from_env()?,
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
pub struct VoyageConfig {
    pub api_key: String,
    pub base_url: String,
    pub embedding_model: String,
    pub output_dimension: u32,
    pub reranker_model: String,
    pub timeout_secs: u64,
    pub rpm: u32,
    pub tpm: u32,
}

impl VoyageConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            api_key: required_env("VOYAGE_API_KEY")?,
            base_url: DEFAULT_VOYAGE_BASE_URL.to_owned(),
            embedding_model: optional_env("VOYAGE_EMBEDDING_MODEL")
                .unwrap_or_else(|| DEFAULT_VOYAGE_EMBEDDING_MODEL.to_owned()),
            output_dimension: optional_env_parse("VOYAGE_OUTPUT_DIMENSION")?
                .unwrap_or(DEFAULT_VOYAGE_OUTPUT_DIMENSION),
            reranker_model: optional_env("VOYAGE_RERANKER_MODEL")
                .unwrap_or_else(|| DEFAULT_VOYAGE_RERANKER_MODEL.to_owned()),
            timeout_secs: DEFAULT_VOYAGE_TIMEOUT_SECS,
            rpm: optional_env_parse("VOYAGE_RPM")?.unwrap_or(DEFAULT_VOYAGE_RPM),
            tpm: optional_env_parse("VOYAGE_TPM")?.unwrap_or(DEFAULT_VOYAGE_TPM),
        })
    }
}

/// Client-side limiter that keeps Voyage embedding requests under the account's
/// rolling 60s RPM and TPM budget so we never hit 429. Shared across all clones.
#[derive(Clone)]
struct VoyageRateLimiter {
    inner: Arc<Mutex<RateLimiterInner>>,
}

struct RateLimiterInner {
    rpm: u32,
    tpm: u32,
    reqs: VecDeque<Instant>,
    toks: VecDeque<(Instant, u32)>,
}

impl VoyageRateLimiter {
    fn new(rpm: u32, tpm: u32) -> Self {
        Self {
            inner: Arc::new(Mutex::new(RateLimiterInner {
                rpm: rpm.max(1),
                tpm: tpm.max(1),
                reqs: VecDeque::new(),
                toks: VecDeque::new(),
            })),
        }
    }

    /// Block until a request of `est_tokens` fits in the rolling 60s budget, then record it.
    async fn acquire(&self, est_tokens: u32) {
        let window = Duration::from_secs(60);
        loop {
            let sleep_for = {
                let mut g = self.inner.lock().await;
                let now = Instant::now();
                while g
                    .reqs
                    .front()
                    .is_some_and(|t| now.duration_since(*t) >= window)
                {
                    g.reqs.pop_front();
                }
                while g
                    .toks
                    .front()
                    .is_some_and(|(t, _)| now.duration_since(*t) >= window)
                {
                    g.toks.pop_front();
                }
                let req_count = g.reqs.len() as u32;
                let tok_sum: u32 = g.toks.iter().map(|(_, n)| *n).sum();
                // Clamp so a single oversized request can't deadlock the limiter.
                let est = est_tokens.clamp(1, g.tpm);
                if req_count < g.rpm && tok_sum + est <= g.tpm {
                    g.reqs.push_back(now);
                    g.toks.push_back((now, est));
                    return;
                }
                let mut wait = Duration::from_millis(250);
                if req_count >= g.rpm
                    && let Some(t) = g.reqs.front()
                {
                    wait = wait.max(window.saturating_sub(now.duration_since(*t)));
                }
                if tok_sum + est > g.tpm
                    && let Some((t, _)) = g.toks.front()
                {
                    wait = wait.max(window.saturating_sub(now.duration_since(*t)));
                }
                wait + Duration::from_millis(50)
            };
            tokio::time::sleep(sleep_for).await;
        }
    }
}

#[derive(Clone)]
pub struct VectorDatabaseClient {
    qdrant: Qdrant,
    voyage_http: HttpClient,
    config: VectorDatabaseConfig,
    rate_limiter: VoyageRateLimiter,
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

        let voyage_http = HttpClient::builder()
            .timeout(Duration::from_secs(config.voyage.timeout_secs))
            .build()
            .context("Failed to create Voyage HTTP client")?;

        let rate_limiter = VoyageRateLimiter::new(config.voyage.rpm, config.voyage.tpm);

        Ok(Self {
            qdrant,
            voyage_http,
            config,
            rate_limiter,
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

    pub async fn embed_text(
        &self,
        input: impl Into<String>,
        input_type: Option<&str>,
    ) -> Result<Vec<f32>> {
        let mut embeddings = self.embed_texts([input.into()], input_type).await?;
        embeddings
            .pop()
            .ok_or_else(|| anyhow!("Voyage returned no embedding for the requested input"))
    }

    pub async fn embed_texts<I, S>(
        &self,
        inputs: I,
        input_type: Option<&str>,
    ) -> Result<Vec<Vec<f32>>>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let input: Vec<String> = inputs.into_iter().map(Into::into).collect();

        // Stay under the Voyage rate limit (rough token estimate: ~4 chars/token).
        let est_tokens: u32 = input
            .iter()
            .map(|s| (s.len() / 4).max(1) as u32)
            .sum::<u32>()
            .max(1);
        self.rate_limiter.acquire(est_tokens).await;

        let payload = VoyageEmbeddingsRequest {
            model: self.config.voyage.embedding_model.clone(),
            input,
            input_type: input_type.map(|s| s.to_owned()),
            output_dimension: self.config.voyage.output_dimension,
            output_dtype: "float".to_owned(),
        };

        let http_response = self
            .voyage_http
            .post(format!("{}/embeddings", self.config.voyage.base_url))
            .bearer_auth(&self.config.voyage.api_key)
            .json(&payload)
            .send()
            .await
            .context("Failed to call Voyage embeddings API")?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            return Err(anyhow!("Voyage embeddings API returned {status}: {body}"));
        }

        let response = http_response
            .json::<VoyageEmbeddingsResponse>()
            .await
            .context("Failed to deserialize Voyage embeddings response")?;

        Ok(response
            .data
            .into_iter()
            .map(|item| item.embedding)
            .collect())
    }

    /// Batch-embed and upsert memory points in a single Voyage request + single
    /// Qdrant upsert. Each item is `(user_id, memory_key, content)`. Used by the
    /// re-embedding migration to stay well under the request-rate budget.
    pub async fn upsert_memories_batch(&self, items: &[(String, String, String)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let texts: Vec<String> = items.iter().map(|(_, _, text)| text.clone()).collect();
        let vectors = self.embed_texts(texts, Some("document")).await?;

        let mut points = Vec::with_capacity(items.len());
        for ((user_id, memory_key, content), dense_vec) in items.iter().zip(vectors) {
            let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(content);
            let named_vectors = NamedVectors::default()
                .add_vector("dense", Vector::new_dense(dense_vec))
                .add_vector("sparse", Vector::new_sparse(sparse_indices, sparse_values));

            let payload: Payload = Payload::try_from(json!({
                "user_id": user_id,
                "memory_key": memory_key,
                "text": content,
            }))
            .context("Failed to build memory batch payload")?;

            points.push(PointStruct::new(
                Self::memory_point_id(user_id, memory_key),
                named_vectors,
                payload,
            ));
        }

        self.qdrant
            .upsert_points(
                UpsertPointsBuilder::new(Self::memories_collection_name(), points).wait(true),
            )
            .await
            .context("Failed to upsert memory batch")?;

        Ok(())
    }

    fn memories_collection_name() -> String {
        std::env::var("QDRANT_MEMORIES_COLLECTION")
            .unwrap_or_else(|_| "tradstry_memories".to_string())
    }

    /// Deterministic Qdrant point id for a memory, so re-indexing the same
    /// (user_id, memory_key) overwrites in place instead of appending a duplicate.
    pub fn memory_point_id(user_id: &str, memory_key: &str) -> String {
        let name = format!("tradstry-memory:{user_id}:{memory_key}");
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, name.as_bytes()).to_string()
    }

    /// Whether a memory point for (user_id, memory_key) is already indexed in Qdrant.
    pub async fn memory_exists(&self, user_id: &str, memory_key: &str) -> Result<bool> {
        let id = Self::memory_point_id(user_id, memory_key);
        let resp = self
            .qdrant
            .get_points(
                GetPointsBuilder::new(Self::memories_collection_name(), vec![id.into()])
                    .with_payload(false)
                    .with_vectors(false),
            )
            .await
            .context("Failed to check memory existence")?;
        Ok(!resp.result.is_empty())
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
                self.config.voyage.output_dimension as u64,
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
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    COLLECTION,
                    field,
                    FieldType::Keyword,
                ))
                .await
                .with_context(|| format!("Failed to create field index on {field}"))?;
        }

        Ok(())
    }

    /// Upserts a single document into `tradstry_hybrid` with both dense and sparse vectors.
    #[allow(clippy::too_many_arguments)]
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
        let dense_vec = self.embed_text(text, Some("document")).await?;
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
            .upsert_points(UpsertPointsBuilder::new("tradstry_hybrid", vec![point]).wait(true))
            .await
            .context("Failed to upsert hybrid point")?;

        Ok(())
    }

    /// Performs a hybrid (dense + sparse via RRF) search, then reranks with Voyage.
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
        let dense_vec: Vec<f32> = self.embed_text(query_text, Some("query")).await?;
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
        let sparse_tuples: Vec<(u32, f32)> =
            sparse_indices.into_iter().zip(sparse_values).collect();

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

        // 6. Rerank with Voyage
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
                self.config.voyage.output_dimension as u64,
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
                .create_field_index(CreateFieldIndexCollectionBuilder::new(
                    &collection,
                    field,
                    FieldType::Keyword,
                ))
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
        let dense_vec = self.embed_text(content, Some("document")).await?;
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

        let point_id = Self::memory_point_id(user_id, memory_key);
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
        let dense_vec: Vec<f32> = self.embed_text(query_text, Some("query")).await?;
        let (sparse_indices, sparse_values) = sparse::text_to_sparse_vector(query_text);

        // 2. Build filter — only by user_id
        let filter = Filter::must(vec![Condition::matches("user_id", user_id.to_string())]);

        // 3. Prefetch amount — fetch more candidates before fusion
        let prefetch_limit = (top_k * 4).max(20);

        let sparse_tuples: Vec<(u32, f32)> =
            sparse_indices.into_iter().zip(sparse_values).collect();

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

    /// Dense-only search in `tradstry_memories` that returns (text, score) pairs.
    /// Used for deduplication where we need the cosine similarity score.
    pub async fn search_memories_scored(
        &self,
        query_text: &str,
        user_id: &str,
        top_k: u64,
    ) -> Result<Vec<(String, f32)>> {
        let dense_vec = self.embed_text(query_text, Some("query")).await?;
        let filter = Filter::must(vec![Condition::matches("user_id", user_id.to_string())]);

        let response = self
            .qdrant
            .query(
                QueryPointsBuilder::new(Self::memories_collection_name())
                    .query(dense_vec)
                    .using("dense")
                    .filter(filter)
                    .limit(top_k)
                    .with_payload(with_payload_selector::SelectorOptions::Enable(true)),
            )
            .await
            .context("Memories scored search failed")?;

        let results: Vec<(String, f32)> = response
            .result
            .iter()
            .filter_map(|pt| {
                let text = pt.payload.get("text").and_then(|v| {
                    if let Some(qdrant_client::qdrant::value::Kind::StringValue(s)) = &v.kind {
                        Some(s.clone())
                    } else {
                        None
                    }
                })?;
                Some((text, pt.score))
            })
            .collect();

        Ok(results)
    }

    pub async fn rerank(
        &self,
        query: impl Into<String>,
        documents: Vec<String>,
        top_k: Option<u32>,
    ) -> Result<Vec<VoyageRerankResult>> {
        let payload = VoyageRerankRequest {
            model: self.config.voyage.reranker_model.clone(),
            query: query.into(),
            documents,
            top_k,
        };

        let http_response = self
            .voyage_http
            .post(format!("{}/rerank", self.config.voyage.base_url))
            .bearer_auth(&self.config.voyage.api_key)
            .json(&payload)
            .send()
            .await
            .context("Failed to call Voyage rerank API")?;

        let status = http_response.status();
        if !status.is_success() {
            let body = http_response.text().await.unwrap_or_default();
            return Err(anyhow!("Voyage rerank API returned {status}: {body}"));
        }

        let response = http_response
            .json::<VoyageRerankResponse>()
            .await
            .context("Failed to deserialize Voyage rerank response")?;

        Ok(response.data)
    }
}

/// Result from a hybrid (dense + sparse) search with Voyage reranking.
#[derive(Clone, Debug, Serialize)]
pub struct HybridSearchResult {
    pub source_id: String,
    pub source_type: String,
    pub text: String,
    pub score: f64,
}

#[derive(Debug, Serialize)]
struct VoyageEmbeddingsRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_type: Option<String>,
    output_dimension: u32,
    output_dtype: String,
}

#[derive(Debug, Deserialize)]
struct VoyageEmbeddingsResponse {
    data: Vec<VoyageEmbeddingData>,
}

#[derive(Debug, Deserialize)]
struct VoyageEmbeddingData {
    embedding: Vec<f32>,
}

#[derive(Debug, Serialize)]
struct VoyageRerankRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    top_k: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct VoyageRerankResponse {
    data: Vec<VoyageRerankResult>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct VoyageRerankResult {
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
