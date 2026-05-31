#![allow(dead_code)]

use std::collections::{HashSet, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow};
use pgvector::HalfVector;
use reqwest::Client as HttpClient;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use tokio::sync::Mutex;

use super::sparse;

const DEFAULT_VOYAGE_BASE_URL: &str = "https://api.voyageai.com/v1";
const DEFAULT_VOYAGE_EMBEDDING_MODEL: &str = "voyage-3.5";
const DEFAULT_VOYAGE_OUTPUT_DIMENSION: u32 = 2048;
const DEFAULT_VOYAGE_RERANKER_MODEL: &str = "rerank-2.5";
const DEFAULT_VOYAGE_TIMEOUT_SECS: u64 = 30;
// Voyage Tier 1 (payment method on file): 2000 RPM / 8M TPM for voyage-3.5.
// These gate embeddings only (rerank is not throttled). Override via the
// VOYAGE_RPM / VOYAGE_TPM env vars if the account moves to a higher tier.
const DEFAULT_VOYAGE_RPM: u32 = 2000;
const DEFAULT_VOYAGE_TPM: u32 = 8_000_000;

/// Vector-DB schema version. Bump this whenever the `ensure_*` DDL changes
/// (new table/column/index/extension) so the bootstrap re-runs once. Mirrors the
/// Turso side's `SCHEMA_VERSION` gate (`service/turso/schema/logic.rs`): a recorded
/// version equal to this means the schema is current, so boot skips all the DDL.
const VECTOR_SCHEMA_VERSION: &str = "1.0";

#[derive(Clone, Debug)]
pub struct VectorDatabaseConfig {
    pub voyage: VoyageConfig,
}

impl VectorDatabaseConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            voyage: VoyageConfig::from_env()?,
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
    pool: PgPool,
    voyage_http: HttpClient,
    config: VectorDatabaseConfig,
    rate_limiter: VoyageRateLimiter,
}

impl VectorDatabaseClient {
    pub fn new(config: VectorDatabaseConfig) -> Result<Self> {
        let postgres_url = required_env("POSTGRES_URL")?;
        let connect_opts: sqlx::postgres::PgConnectOptions = postgres_url
            .parse()
            .context("Failed to parse POSTGRES_URL")?;
        // Suppress server NOTICE messages (e.g. "... already exists, skipping"
        // from the idempotent CREATE/ALTER IF NOT EXISTS ensure_* DDL run on
        // boot) per session — mirrors the langgraph checkpoint/store pools. Not
        // a log filter: Postgres simply stops sending notices on this pool.
        // `connect_lazy_with` stays synchronous; the connection (and this SET)
        // happens on the first query.
        //
        // `min_connections(1)` keeps the connection opened by the boot
        // `health_check` alive permanently (default min is 0, so it would be
        // reaped after the idle timeout — meaning a later first chat reopens a
        // cold connection and pays the TCP+TLS handshake). `test_before_acquire`
        // pings that kept-alive connection before handing it out, so a server
        // that dropped an idle connection is detected and replaced up front
        // rather than mid-query.
        let pool = PgPoolOptions::new()
            .min_connections(1)
            .test_before_acquire(true)
            .after_connect(|conn, _meta| {
                Box::pin(async move {
                    use sqlx::Executor;
                    conn.execute("SET client_min_messages = WARNING").await?;
                    // RAG recall tuning (pgvector 0.8.2). Session GUCs applied to
                    // every search on this pooled connection — no schema/reindex.
                    // B1: lift the default ef_search=40 cap so the index can return
                    // the full top_k*4 prefetch (<=80) instead of silently truncating.
                    conn.execute("SET hnsw.ef_search = 100").await?;
                    // B2: (user_id, account_id) filtering is applied AFTER the ANN
                    // scan; iterative scan keeps scanning past the filter until enough
                    // real matches are found. relaxed_order (not strict) is fine — the
                    // candidates are RRF-fused + Voyage-reranked downstream, so exact
                    // ANN distance order does not matter.
                    conn.execute("SET hnsw.iterative_scan = 'relaxed_order'")
                        .await?;
                    // Let relaxed iterative scan hold more candidates (default 1 limits it).
                    conn.execute("SET hnsw.scan_mem_multiplier = 2").await?;
                    Ok(())
                })
            })
            .connect_lazy_with(connect_opts);

        let voyage_http = HttpClient::builder()
            .timeout(Duration::from_secs(config.voyage.timeout_secs))
            .build()
            .context("Failed to create Voyage HTTP client")?;

        let rate_limiter = VoyageRateLimiter::new(config.voyage.rpm, config.voyage.tpm);

        Ok(Self {
            pool,
            voyage_http,
            config,
            rate_limiter,
        })
    }

    pub fn from_env() -> Result<Self> {
        Self::new(VectorDatabaseConfig::from_env()?)
    }

    pub fn config(&self) -> &VectorDatabaseConfig {
        &self.config
    }

    /// Verifies the Postgres connection is reachable.
    pub async fn health_check(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .context("Postgres health check failed")?;
        Ok(())
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

        // Retry transient failures: a connection/timeout error (send fails) or a
        // 429/5xx response. Other 4xx (auth, bad request) are permanent → fail
        // fast. Without this, a single network blip kills the whole call (e.g.
        // semantic_search returning no results in chat).
        const MAX_ATTEMPTS: u32 = 3;
        let url = format!("{}/embeddings", self.config.voyage.base_url);
        let http_response = {
            let mut attempt = 0u32;
            loop {
                attempt += 1;
                let send_result = self
                    .voyage_http
                    .post(&url)
                    .bearer_auth(&self.config.voyage.api_key)
                    .json(&payload)
                    .send()
                    .await;

                match send_result {
                    Ok(resp) if resp.status().is_success() => break resp,
                    Ok(resp) => {
                        let status = resp.status();
                        let retryable = status.as_u16() == 429 || status.is_server_error();
                        let body = resp.text().await.unwrap_or_default();
                        if retryable && attempt < MAX_ATTEMPTS {
                            log::warn!(
                                "Voyage embeddings API {status} (attempt {attempt}/{MAX_ATTEMPTS}), retrying: {body}"
                            );
                        } else {
                            return Err(anyhow!("Voyage embeddings API returned {status}: {body}"));
                        }
                    }
                    Err(error) => {
                        if attempt < MAX_ATTEMPTS {
                            log::warn!(
                                "Voyage embeddings request failed (attempt {attempt}/{MAX_ATTEMPTS}): {error}"
                            );
                        } else {
                            return Err(anyhow::Error::new(error)
                                .context("Failed to call Voyage embeddings API after retries"));
                        }
                    }
                }

                // Exponential-ish backoff: 300ms, 600ms.
                tokio::time::sleep(Duration::from_millis(300 * 2u64.pow(attempt - 1))).await;
            }
        };

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

    /// Batch-embed and upsert memory rows in a single Voyage request. Each item is
    /// `(user_id, memory_key, content)`. Batches the embed call to stay well under
    /// the request-rate budget.
    pub async fn upsert_memories_batch(&self, items: &[(String, String, String)]) -> Result<()> {
        if items.is_empty() {
            return Ok(());
        }

        let texts: Vec<String> = items.iter().map(|(_, _, text)| text.clone()).collect();
        let vectors = self.embed_texts(texts, Some("document")).await?;

        for ((user_id, memory_key, content), dense_vec) in items.iter().zip(vectors) {
            self.insert_memory_row(user_id, memory_key, content, dense_vec)
                .await?;
        }

        Ok(())
    }

    /// Deterministic id for a memory, so re-indexing the same (user_id, memory_key)
    /// overwrites in place instead of appending a duplicate.
    pub fn memory_point_id(user_id: &str, memory_key: &str) -> String {
        let name = format!("tradstry-memory:{user_id}:{memory_key}");
        uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, name.as_bytes()).to_string()
    }

    async fn insert_memory_row(
        &self,
        user_id: &str,
        memory_key: &str,
        content: &str,
        dense_vec: Vec<f32>,
    ) -> Result<()> {
        let (sparse_idx, sparse_val) = sparse::text_to_sparse_vector(content);
        let sparse_idx_i64: Vec<i64> = sparse_idx.iter().map(|&v| v as i64).collect();
        let dense = HalfVector::from_f32_slice(&dense_vec);

        sqlx::query(
            "INSERT INTO vector_memories (id, user_id, memory_key, content, dense, sparse_idx, sparse_val, bm25_text) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
             ON CONFLICT (user_id, memory_key) DO UPDATE SET \
             content = EXCLUDED.content, dense = EXCLUDED.dense, \
             sparse_idx = EXCLUDED.sparse_idx, sparse_val = EXCLUDED.sparse_val, \
             bm25_text = EXCLUDED.bm25_text",
        )
        .bind(Self::memory_point_id(user_id, memory_key))
        .bind(user_id)
        .bind(memory_key)
        .bind(content)
        .bind(dense)
        .bind(&sparse_idx_i64)
        .bind(&sparse_val)
        .bind(content)
        .execute(&self.pool)
        .await
        .context("Failed to upsert memory row")?;

        Ok(())
    }

    /// Whether a memory row for (user_id, memory_key) is already indexed.
    pub async fn memory_exists(&self, user_id: &str, memory_key: &str) -> Result<bool> {
        let row: Option<(i32,)> =
            sqlx::query_as("SELECT 1 FROM vector_memories WHERE user_id = $1 AND memory_key = $2")
                .bind(user_id)
                .bind(memory_key)
                .fetch_optional(&self.pool)
                .await
                .context("Failed to check memory existence")?;
        Ok(row.is_some())
    }

    /// Creates the `vector_documents` table (with pgvector extension + indexes).
    /// Idempotent — safe to call on every startup.
    /// Version-gated schema bootstrap, mirroring the Turso `migrate` gate
    /// (`service/turso/schema/logic.rs`). Reads the recorded schema version; if it
    /// already matches `VECTOR_SCHEMA_VERSION`, returns immediately — skipping all
    /// the extension/table/index DDL (so a normal boot is two round-trips, not ~19).
    /// Only a fresh DB (no version row) or a version bump runs the full `ensure_*`
    /// DDL, after which the version is stamped.
    pub async fn ensure_schema(&self) -> Result<()> {
        if self.applied_schema_version().await?.as_deref() == Some(VECTOR_SCHEMA_VERSION) {
            log::info!("Vector DB schema is up to date at v{VECTOR_SCHEMA_VERSION}");
            return Ok(());
        }

        log::info!("Bootstrapping vector DB schema to v{VECTOR_SCHEMA_VERSION}");
        self.ensure_extensions().await?;
        self.ensure_hybrid_collection().await?;
        self.ensure_memories_collection().await?;
        self.stamp_schema_version().await?;
        Ok(())
    }

    /// Create the Postgres extensions once. The `vector` (pgvector) and `pg_search`
    /// (ParadeDB BM25) extensions are a precondition for both collections' tables
    /// and indexes; hoisting them here avoids the duplicate `CREATE EXTENSION` each
    /// `ensure_*` body used to run. (Provisioning — `scripts/postgres/postgres.sh` —
    /// also creates these; this is the app-side self-bootstrap for a fresh DB.)
    async fn ensure_extensions(&self) -> Result<()> {
        sqlx::query("CREATE EXTENSION IF NOT EXISTS vector")
            .execute(&self.pool)
            .await
            .context("Failed to create vector extension")?;
        sqlx::query("CREATE EXTENSION IF NOT EXISTS pg_search")
            .execute(&self.pool)
            .await
            .context("Failed to create pg_search extension")?;
        Ok(())
    }

    /// Ensure the version table exists (one cheap idempotent statement, the only
    /// DDL a current-schema boot runs) and return the recorded version, if any.
    async fn applied_schema_version(&self) -> Result<Option<String>> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS vector_schema_version (\
                id      INTEGER PRIMARY KEY,\
                version TEXT NOT NULL\
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to ensure vector_schema_version table")?;

        let row: Option<(String,)> =
            sqlx::query_as("SELECT version FROM vector_schema_version WHERE id = 1")
                .fetch_optional(&self.pool)
                .await
                .context("Failed to read vector_schema_version")?;
        Ok(row.map(|(v,)| v))
    }

    /// Record the current schema version after a successful bootstrap.
    async fn stamp_schema_version(&self) -> Result<()> {
        sqlx::query(
            "INSERT INTO vector_schema_version (id, version) VALUES (1, $1) \
             ON CONFLICT (id) DO UPDATE SET version = EXCLUDED.version",
        )
        .bind(VECTOR_SCHEMA_VERSION)
        .execute(&self.pool)
        .await
        .context("Failed to stamp vector_schema_version")?;
        Ok(())
    }

    /// Create the `vector_documents` / `vector_parents` tables and their indexes.
    /// Assumes the `vector` + `pg_search` extensions already exist (see
    /// `ensure_extensions`, run first by `ensure_schema`).
    pub async fn ensure_hybrid_collection(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS vector_documents (\
                id          TEXT PRIMARY KEY,\
                user_id     TEXT NOT NULL,\
                account_id  TEXT NOT NULL,\
                source_type TEXT NOT NULL,\
                source_id   TEXT NOT NULL,\
                title       TEXT NOT NULL DEFAULT '',\
                content     TEXT NOT NULL,\
                created_at  TEXT NOT NULL,\
                dense       halfvec(2048) NOT NULL,\
                sparse_idx  bigint[] NOT NULL,\
                sparse_val  real[] NOT NULL,\
                source_content_hash TEXT NOT NULL DEFAULT '',\
                parent_id   TEXT\
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create vector_documents table")?;

        // Idempotent backfill for tables created before `source_content_hash` existed.
        sqlx::query(
            "ALTER TABLE vector_documents ADD COLUMN IF NOT EXISTS source_content_hash TEXT NOT NULL DEFAULT ''",
        )
        .execute(&self.pool)
        .await
        .context("Failed to add source_content_hash column")?;

        // Idempotent backfill for tables created before `parent_id` existed.
        sqlx::query("ALTER TABLE vector_documents ADD COLUMN IF NOT EXISTS parent_id TEXT")
            .execute(&self.pool)
            .await
            .context("Failed to add parent_id column")?;

        // Parent sections: small child chunks are the embedded unit and each child
        // links (via `parent_id`) to a fuller parent section returned at search time.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS vector_parents (\
                id          TEXT PRIMARY KEY,\
                user_id     TEXT NOT NULL,\
                account_id  TEXT NOT NULL,\
                source_type TEXT NOT NULL,\
                source_id   TEXT NOT NULL,\
                title       TEXT NOT NULL DEFAULT '',\
                content     TEXT NOT NULL,\
                created_at  TEXT NOT NULL\
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create vector_parents table")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecparents_account ON vector_parents (user_id, account_id)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecparents_account")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecdocs_account ON vector_documents (user_id, account_id)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecdocs_account")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecdocs_source ON vector_documents (source_type, source_id)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecdocs_source")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecdocs_dense ON vector_documents \
             USING hnsw (dense halfvec_cosine_ops) WITH (m = 32, ef_construction = 200)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecdocs_dense")?;

        sqlx::query(
            "ALTER TABLE vector_documents ADD COLUMN IF NOT EXISTS bm25_text TEXT NOT NULL DEFAULT ''",
        )
        .execute(&self.pool)
        .await
        .context("Failed to add bm25_text column")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecdocs_bm25 ON vector_documents \
             USING bm25 (id, bm25_text) WITH (key_field='id')",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecdocs_bm25")?;

        // Per-document LLM context-blurb cache, keyed by the source doc's content
        // hash + chunk index, so unchanged docs skip the Gemini call on reindex.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS vector_context_cache (\
                content_hash TEXT NOT NULL,\
                chunk_index  INTEGER NOT NULL,\
                blurb        TEXT NOT NULL,\
                PRIMARY KEY (content_hash, chunk_index)\
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create vector_context_cache table")?;

        Ok(())
    }

    /// Fetch cached LLM context blurbs for a source document, keyed by chunk index.
    pub async fn get_context_blurbs(
        &self,
        content_hash: &str,
    ) -> Result<std::collections::HashMap<i32, String>> {
        let rows = sqlx::query_as::<_, (i32, String)>(
            "SELECT chunk_index, blurb FROM vector_context_cache WHERE content_hash=$1",
        )
        .bind(content_hash)
        .fetch_all(&self.pool)
        .await
        .context("get_context_blurbs failed")?;
        Ok(rows.into_iter().collect())
    }

    /// Upsert LLM context blurbs for a source document, one row per chunk index.
    pub async fn put_context_blurbs(
        &self,
        content_hash: &str,
        blurbs: &[(i32, String)],
    ) -> Result<()> {
        for (idx, blurb) in blurbs {
            sqlx::query(
                "INSERT INTO vector_context_cache (content_hash, chunk_index, blurb) VALUES ($1,$2,$3) \
                 ON CONFLICT (content_hash, chunk_index) DO UPDATE SET blurb = EXCLUDED.blurb",
            )
            .bind(content_hash)
            .bind(idx)
            .bind(blurb)
            .execute(&self.pool)
            .await
            .context("put_context_blurbs failed")?;
        }
        Ok(())
    }

    /// Upserts a single document into `vector_documents` with dense + sparse vectors.
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
        let (sparse_idx, sparse_val) = sparse::text_to_sparse_vector(text);
        let sparse_idx_i64: Vec<i64> = sparse_idx.iter().map(|&v| v as i64).collect();
        let dense = HalfVector::from_f32_slice(&dense_vec);

        sqlx::query(
            "INSERT INTO vector_documents \
             (id, user_id, account_id, source_type, source_id, title, content, created_at, dense, sparse_idx, sparse_val) \
             VALUES ($1, $2, $3, $4, $5, '', $6, $7, $8, $9, $10) \
             ON CONFLICT (id) DO UPDATE SET \
             user_id = EXCLUDED.user_id, account_id = EXCLUDED.account_id, \
             source_type = EXCLUDED.source_type, source_id = EXCLUDED.source_id, \
             content = EXCLUDED.content, created_at = EXCLUDED.created_at, \
             dense = EXCLUDED.dense, sparse_idx = EXCLUDED.sparse_idx, sparse_val = EXCLUDED.sparse_val",
        )
        .bind(point_id)
        .bind(user_id)
        .bind(account_id)
        .bind(source_type)
        .bind(source_id)
        .bind(text)
        .bind(created_at)
        .bind(dense)
        .bind(&sparse_idx_i64)
        .bind(&sparse_val)
        .execute(&self.pool)
        .await
        .context("Failed to upsert hybrid document")?;

        Ok(())
    }

    /// Upserts a batch of pre-embedded documents into `vector_documents`. The caller
    /// has already computed the dense vectors (to batch Voyage calls); sparse vectors
    /// are computed here from `content`.
    pub async fn upsert_documents(&self, rows: &[VectorDocumentUpsert]) -> Result<()> {
        for row in rows {
            let (sparse_idx, sparse_val) = sparse::text_to_sparse_vector(&row.embed_text);
            let sparse_idx_i64: Vec<i64> = sparse_idx.iter().map(|&v| v as i64).collect();
            let dense = HalfVector::from_f32_slice(&row.dense);

            sqlx::query(
                "INSERT INTO vector_documents \
                 (id, user_id, account_id, source_type, source_id, title, content, created_at, dense, sparse_idx, sparse_val, source_content_hash, parent_id, bm25_text) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14) \
                 ON CONFLICT (id) DO UPDATE SET \
                 user_id = EXCLUDED.user_id, account_id = EXCLUDED.account_id, \
                 source_type = EXCLUDED.source_type, source_id = EXCLUDED.source_id, \
                 title = EXCLUDED.title, content = EXCLUDED.content, created_at = EXCLUDED.created_at, \
                 dense = EXCLUDED.dense, sparse_idx = EXCLUDED.sparse_idx, sparse_val = EXCLUDED.sparse_val, \
                 source_content_hash = EXCLUDED.source_content_hash, parent_id = EXCLUDED.parent_id, \
                 bm25_text = EXCLUDED.bm25_text",
            )
            .bind(&row.id)
            .bind(&row.user_id)
            .bind(&row.account_id)
            .bind(&row.source_type)
            .bind(&row.source_id)
            .bind(&row.title)
            .bind(&row.content)
            .bind(&row.created_at)
            .bind(dense)
            .bind(&sparse_idx_i64)
            .bind(&sparse_val)
            .bind(&row.source_content_hash)
            .bind(&row.parent_id)
            .bind(&row.bm25_text)
            .execute(&self.pool)
            .await
            .context("Failed to upsert document row")?;
        }

        Ok(())
    }

    /// Deletes all document vectors for a (user_id, account_id) pair.
    pub async fn delete_documents_by_account(&self, user_id: &str, account_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM vector_documents WHERE user_id = $1 AND account_id = $2")
            .bind(user_id)
            .bind(account_id)
            .execute(&self.pool)
            .await
            .context("Failed to delete documents by account")?;
        Ok(())
    }

    /// Delete all chunk rows for one source document (used by incremental reindex).
    pub async fn delete_documents_by_source(
        &self,
        user_id: &str,
        account_id: &str,
        source_type: &str,
        source_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM vector_documents WHERE user_id=$1 AND account_id=$2 AND source_type=$3 AND source_id=$4",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(source_type)
        .bind(source_id)
        .execute(&self.pool)
        .await
        .context("delete_documents_by_source failed")?;
        Ok(())
    }

    /// Delete all chunk rows for one source document by id only (used by the
    /// incremental reindexer for sources that have been removed entirely).
    pub async fn delete_documents_by_source_id(
        &self,
        user_id: &str,
        account_id: &str,
        source_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM vector_documents WHERE user_id=$1 AND account_id=$2 AND source_id=$3",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(source_id)
        .execute(&self.pool)
        .await
        .context("delete_documents_by_source_id failed")?;
        Ok(())
    }

    /// Upserts a batch of parent sections into `vector_parents`. Parents hold the
    /// fuller section/document text linked to child chunks via `parent_id`.
    pub async fn upsert_parents(&self, rows: &[VectorParentUpsert]) -> Result<()> {
        for row in rows {
            sqlx::query(
                "INSERT INTO vector_parents \
                 (id, user_id, account_id, source_type, source_id, title, content, created_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                 ON CONFLICT (id) DO UPDATE SET \
                 user_id = EXCLUDED.user_id, account_id = EXCLUDED.account_id, \
                 source_type = EXCLUDED.source_type, source_id = EXCLUDED.source_id, \
                 title = EXCLUDED.title, content = EXCLUDED.content, created_at = EXCLUDED.created_at",
            )
            .bind(&row.id)
            .bind(&row.user_id)
            .bind(&row.account_id)
            .bind(&row.source_type)
            .bind(&row.source_id)
            .bind(&row.title)
            .bind(&row.content)
            .bind(&row.created_at)
            .execute(&self.pool)
            .await
            .context("Failed to upsert parent row")?;
        }

        Ok(())
    }

    /// Delete all parent rows for one source document (mirrors
    /// `delete_documents_by_source`, used by the incremental reindexer).
    pub async fn delete_parents_by_source(
        &self,
        user_id: &str,
        account_id: &str,
        source_type: &str,
        source_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM vector_parents WHERE user_id=$1 AND account_id=$2 AND source_type=$3 AND source_id=$4",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(source_type)
        .bind(source_id)
        .execute(&self.pool)
        .await
        .context("delete_parents_by_source failed")?;
        Ok(())
    }

    /// Delete all parent rows for one source document by id only (mirrors
    /// `delete_documents_by_source_id`, for sources removed entirely).
    pub async fn delete_parents_by_source_id(
        &self,
        user_id: &str,
        account_id: &str,
        source_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM vector_parents WHERE user_id=$1 AND account_id=$2 AND source_id=$3",
        )
        .bind(user_id)
        .bind(account_id)
        .bind(source_id)
        .execute(&self.pool)
        .await
        .context("delete_parents_by_source_id failed")?;
        Ok(())
    }

    /// Returns the (source_id, source_content_hash) pairs currently indexed for an
    /// account, so the reindexer can skip unchanged documents.
    pub async fn indexed_source_hashes(
        &self,
        user_id: &str,
        account_id: &str,
    ) -> Result<std::collections::HashMap<String, String>> {
        let rows = sqlx::query_as::<_, (String, String)>(
            "SELECT DISTINCT source_id, source_content_hash FROM vector_documents WHERE user_id=$1 AND account_id=$2",
        )
        .bind(user_id)
        .bind(account_id)
        .fetch_all(&self.pool)
        .await
        .context("indexed_source_hashes failed")?;
        Ok(rows.into_iter().collect())
    }

    /// Dense ANN + BM25 union → SQL RRF fusion. Takes a pre-computed dense embedding
    /// and the raw query text for BM25. Returns candidates in fused order.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn gather_candidates(
        &self,
        dense_vec: &[f32],
        query_text: &str,
        user_id: &str,
        account_id: &str,
        date_from: Option<&str>,
        date_to: Option<&str>,
        prefetch: i64,
    ) -> Result<Vec<DocCandidateRow>> {
        let query_dense = HalfVector::from_f32_slice(dense_vec);
        let pf = prefetch.to_string();

        // Build the date clause (identical for both CTEs).
        let date_clause = match (date_from, date_to) {
            (Some(_), Some(_)) => " AND created_at BETWEEN $4 AND $5".to_owned(),
            (Some(_), None) => " AND created_at >= $4".to_owned(),
            (None, Some(_)) => " AND created_at <= $4".to_owned(),
            (None, None) => String::new(),
        };

        if query_text.trim().is_empty() {
            // Dense-only path: BM25 with an empty string would error in pg_search.
            // SAFETY: only string literals + numeric `pf`; all user input is bound.
            let sql = format!(
                "SELECT content, source_type, source_id, title, parent_id \
                 FROM vector_documents WHERE user_id = $2 AND account_id = $3{date_clause} \
                 ORDER BY dense <=> $1 LIMIT {pf}",
            );
            let mut q = sqlx::query_as::<_, DocCandidateRow>(sqlx::AssertSqlSafe(sql))
                .bind(query_dense)
                .bind(user_id)
                .bind(account_id);
            match (date_from, date_to) {
                (Some(from), Some(to)) => q = q.bind(from).bind(to),
                (Some(from), None) => q = q.bind(from),
                (None, Some(to)) => q = q.bind(to),
                (None, None) => {}
            }
            return q
                .fetch_all(&self.pool)
                .await
                .context("gather_candidates dense-only query failed");
        }

        // Non-empty query: dense CTE ∪ BM25 CTE → RRF in SQL.
        // Bind order: $1=dense, $2=user_id, $3=account_id, $4/$5=date (if any), $N=query_text.
        let qn = match (date_from, date_to) {
            (Some(_), Some(_)) => "$6",
            (Some(_), None) | (None, Some(_)) => "$5",
            (None, None) => "$4",
        };

        let sql = format!(
            "WITH dense AS (\
               SELECT id, ROW_NUMBER() OVER (ORDER BY dense <=> $1) AS rnk \
               FROM vector_documents \
               WHERE user_id = $2 AND account_id = $3{date_clause} \
               ORDER BY dense <=> $1 LIMIT {pf}\
             ), \
             lex AS (\
               SELECT id, ROW_NUMBER() OVER (ORDER BY paradedb.score(id) DESC) AS rnk \
               FROM vector_documents \
               WHERE user_id = $2 AND account_id = $3{date_clause} AND bm25_text @@@ {qn} \
               LIMIT {pf}\
             ) \
             SELECT d.content, d.source_type, d.source_id, d.title, d.parent_id \
             FROM dense FULL OUTER JOIN lex USING (id) \
             JOIN vector_documents d ON d.id = COALESCE(dense.id, lex.id) \
             ORDER BY (COALESCE(1.0/(60+dense.rnk),0) + COALESCE(1.0/(60+lex.rnk),0)) DESC \
             LIMIT {pf}",
        );

        // SAFETY: only string literals + numeric `pf`; all user input is bound.
        let mut q = sqlx::query_as::<_, DocCandidateRow>(sqlx::AssertSqlSafe(sql))
            .bind(query_dense)
            .bind(user_id)
            .bind(account_id);
        match (date_from, date_to) {
            (Some(from), Some(to)) => q = q.bind(from).bind(to),
            (Some(from), None) => q = q.bind(from),
            (None, Some(to)) => q = q.bind(to),
            (None, None) => {}
        }
        q = q.bind(query_text);

        q.fetch_all(&self.pool)
            .await
            .context("gather_candidates query failed")
    }

    /// Voyage rerank of fused candidates + parent expansion/dedup. Returns the
    /// top results (parent content where available, else the child chunk).
    pub(crate) async fn rerank_and_expand(
        &self,
        query_text: &str,
        candidates: Vec<DocCandidateRow>,
        top_k: u32,
    ) -> Result<Vec<HybridSearchResult>> {
        if candidates.is_empty() {
            return Ok(vec![]);
        }

        let texts: Vec<String> = candidates.iter().map(|m| m.content.clone()).collect();
        let rerank_results = self
            .rerank(query_text, texts, Some(top_k))
            .await
            .context("rerank_and_expand reranking failed")?;

        let selected: Vec<(&DocCandidateRow, f64)> = rerank_results
            .into_iter()
            .filter_map(|r| {
                candidates
                    .get(r.index)
                    .map(|m| (m, r.relevance_score as f64))
            })
            .collect();

        let mut parent_ids: Vec<String> = Vec::new();
        let mut seen_parent_ids: HashSet<&str> = HashSet::new();
        for (child, _) in &selected {
            if let Some(pid) = &child.parent_id
                && seen_parent_ids.insert(pid.as_str())
            {
                parent_ids.push(pid.clone());
            }
        }

        let parents: std::collections::HashMap<String, ParentRow> = if parent_ids.is_empty() {
            std::collections::HashMap::new()
        } else {
            let rows: Vec<ParentRow> = sqlx::query_as(
                "SELECT id, source_type, source_id, title, content FROM vector_parents WHERE id = ANY($1)",
            )
            .bind(&parent_ids)
            .fetch_all(&self.pool)
            .await
            .context("rerank_and_expand parent expansion query failed")?;
            rows.into_iter().map(|p| (p.id.clone(), p)).collect()
        };

        let mut results: Vec<HybridSearchResult> = Vec::with_capacity(selected.len());
        let mut emitted_parents: HashSet<&str> = HashSet::new();
        let mut emitted_children: HashSet<(String, String)> = HashSet::new();
        for (child, score) in &selected {
            match child.parent_id.as_deref().and_then(|pid| parents.get(pid)) {
                Some(parent) => {
                    if !emitted_parents.insert(parent.id.as_str()) {
                        continue;
                    }
                    results.push(HybridSearchResult {
                        source_id: parent.source_id.clone(),
                        source_type: parent.source_type.clone(),
                        title: parent.title.clone(),
                        text: parent.content.clone(),
                        score: *score,
                    });
                }
                None => {
                    let key = (child.source_id.clone(), child.content.clone());
                    if !emitted_children.insert(key) {
                        continue;
                    }
                    results.push(HybridSearchResult {
                        source_id: child.source_id.clone(),
                        source_type: child.source_type.clone(),
                        title: child.title.clone(),
                        text: child.content.clone(),
                        score: *score,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Performs a hybrid (dense ANN + BM25 → SQL RRF) search over `vector_documents`,
    /// then reranks the fused candidates with Voyage.
    pub async fn hybrid_search(
        &self,
        query_text: &str,
        user_id: &str,
        account_id: &str,
        date_from: Option<&str>,
        date_to: Option<&str>,
        top_k: u64,
    ) -> Result<Vec<HybridSearchResult>> {
        let dense_vec = self.embed_text(query_text, Some("query")).await?;
        // B6: feed the reranker ~100 candidates (aligned with hnsw.ef_search = 100).
        let prefetch = (top_k * 4).max(100) as i64;
        let candidates = self
            .gather_candidates(
                &dense_vec, query_text, user_id, account_id, date_from, date_to, prefetch,
            )
            .await?;
        self.rerank_and_expand(query_text, candidates, top_k as u32)
            .await
    }

    /// Creates the `vector_memories` table (with pgvector extension + indexes).
    /// Idempotent — safe to call on every startup.
    /// Create the `vector_memories` table and its indexes. Assumes the `vector` +
    /// `pg_search` extensions already exist (see `ensure_extensions`, run first by
    /// `ensure_schema`).
    pub async fn ensure_memories_collection(&self) -> Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS vector_memories (\
                id          TEXT PRIMARY KEY,\
                user_id     TEXT NOT NULL,\
                memory_key  TEXT NOT NULL,\
                content     TEXT NOT NULL,\
                dense       halfvec(2048) NOT NULL,\
                sparse_idx  bigint[] NOT NULL,\
                sparse_val  real[] NOT NULL,\
                UNIQUE (user_id, memory_key)\
            )",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create vector_memories table")?;

        sqlx::query("CREATE INDEX IF NOT EXISTS idx_vecmem_user ON vector_memories (user_id)")
            .execute(&self.pool)
            .await
            .context("Failed to create idx_vecmem_user")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecmem_dense ON vector_memories \
             USING hnsw (dense halfvec_cosine_ops) WITH (m = 32, ef_construction = 200)",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecmem_dense")?;

        sqlx::query(
            "ALTER TABLE vector_memories ADD COLUMN IF NOT EXISTS bm25_text TEXT NOT NULL DEFAULT ''",
        )
        .execute(&self.pool)
        .await
        .context("Failed to add bm25_text column to vector_memories")?;

        sqlx::query(
            "CREATE INDEX IF NOT EXISTS idx_vecmem_bm25 ON vector_memories \
             USING bm25 (id, bm25_text) WITH (key_field='id')",
        )
        .execute(&self.pool)
        .await
        .context("Failed to create idx_vecmem_bm25")?;

        Ok(())
    }

    /// Upserts a single memory into `vector_memories` with dense + sparse vectors.
    pub async fn upsert_memory(
        &self,
        user_id: &str,
        memory_key: &str,
        content: &str,
    ) -> Result<()> {
        let dense_vec = self.embed_text(content, Some("document")).await?;
        self.insert_memory_row(user_id, memory_key, content, dense_vec)
            .await
    }

    /// Searches `vector_memories` using dense∪BM25 → SQL RRF, filtered by `user_id`.
    /// Returns the `content` of the top_k fused results. No rerank.
    pub async fn search_memories(
        &self,
        query_text: &str,
        user_id: &str,
        top_k: u64,
    ) -> Result<Vec<String>> {
        let dense_vec: Vec<f32> = self.embed_text(query_text, Some("query")).await?;
        let query_dense = HalfVector::from_f32_slice(&dense_vec);
        let pf = (top_k * 4).max(20);
        let pf_str = pf.to_string();
        let topk_str = top_k.to_string();

        if query_text.trim().is_empty() {
            // Dense-only path when query is empty.
            let sql = format!(
                "SELECT content FROM vector_memories WHERE user_id = $2 \
                 ORDER BY dense <=> $1 LIMIT {topk_str}",
            );
            let rows: Vec<(String,)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
                .bind(query_dense)
                .bind(user_id)
                .fetch_all(&self.pool)
                .await
                .context("Memories dense-only search failed")?;
            return Ok(rows.into_iter().map(|(c,)| c).collect());
        }

        // Dense ∪ BM25 → SQL RRF. Bind: $1=dense, $2=user_id, $3=query_text.
        // SAFETY: only string literals + numeric `pf_str`/`topk_str`; user input bound.
        let sql = format!(
            "WITH dense AS (\
               SELECT id, content, ROW_NUMBER() OVER (ORDER BY dense <=> $1) AS rnk \
               FROM vector_memories WHERE user_id = $2 ORDER BY dense <=> $1 LIMIT {pf_str}\
             ), \
             lex AS (\
               SELECT id, ROW_NUMBER() OVER (ORDER BY paradedb.score(id) DESC) AS rnk \
               FROM vector_memories WHERE user_id = $2 AND bm25_text @@@ $3 LIMIT {pf_str}\
             ) \
             SELECT m.content \
             FROM dense FULL OUTER JOIN lex USING (id) \
             JOIN vector_memories m ON m.id = COALESCE(dense.id, lex.id) \
             ORDER BY (COALESCE(1.0/(60+dense.rnk),0) + COALESCE(1.0/(60+lex.rnk),0)) DESC \
             LIMIT {topk_str}",
        );

        let rows: Vec<(String,)> = sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(query_dense)
            .bind(user_id)
            .bind(query_text)
            .fetch_all(&self.pool)
            .await
            .context("Memories search query failed")?;

        Ok(rows.into_iter().map(|(c,)| c).collect())
    }

    /// Dense-only cosine search in `vector_memories` that returns (content, similarity)
    /// pairs (similarity = 1 - cosine distance, so higher = more similar). Used for dedup.
    pub async fn search_memories_scored(
        &self,
        query_text: &str,
        user_id: &str,
        top_k: u64,
    ) -> Result<Vec<(String, f32)>> {
        let dense_vec = self.embed_text(query_text, Some("query")).await?;
        let query_dense = HalfVector::from_f32_slice(&dense_vec);

        let rows: Vec<MemoryScoredRow> = sqlx::query_as(
            "SELECT content, (dense <=> $1) AS dist FROM vector_memories \
             WHERE user_id = $2 ORDER BY dense <=> $1 LIMIT $3",
        )
        .bind(query_dense)
        .bind(user_id)
        .bind(top_k as i64)
        .fetch_all(&self.pool)
        .await
        .context("Memories scored search failed")?;

        Ok(rows
            .into_iter()
            .map(|r| (r.content, 1.0 - r.dist as f32))
            .collect())
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
    pub title: String,
    pub text: String,
    pub score: f64,
}

/// A pre-embedded document row to upsert into `vector_documents`. The dense vector
/// is supplied by the caller (so Voyage calls can be batched); sparse is computed
/// on write from `embed_text` (the enriched text actually embedded), while `content`
/// stores the raw chunk used for display/citation/rerank.
#[derive(Clone, Debug)]
pub struct VectorDocumentUpsert {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub embed_text: String,
    pub bm25_text: String,
    pub created_at: String,
    pub dense: Vec<f32>,
    pub source_content_hash: String,
    pub parent_id: Option<String>,
}

/// A parent section to upsert into `vector_parents`. Holds the fuller text (a
/// heading-section for long notes, or the whole doc for short docs) that is
/// returned at search time when one of its child chunks matches.
#[derive(Clone, Debug)]
pub struct VectorParentUpsert {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub content: String,
    pub created_at: String,
}

// --- Internal candidate row types fetched from Postgres ---

#[derive(Clone, sqlx::FromRow)]
pub(crate) struct DocCandidateRow {
    pub(crate) content: String,
    pub(crate) source_type: String,
    pub(crate) source_id: String,
    title: String,
    parent_id: Option<String>,
}

#[derive(sqlx::FromRow)]
struct ParentRow {
    id: String,
    source_type: String,
    source_id: String,
    title: String,
    content: String,
}

#[derive(sqlx::FromRow)]
struct MemoryCandidateRow {
    content: String,
    sparse_idx: Vec<i64>,
    sparse_val: Vec<f32>,
    dist: f64,
}

#[derive(sqlx::FromRow)]
struct MemoryScoredRow {
    content: String,
    dist: f64,
}

/// Candidates that can participate in RRF fusion expose their dense distance and
/// stored sparse vector.
trait FusionCandidate {
    fn dense_dist(&self) -> f64;
    fn sparse(&self) -> (&[i64], &[f32]);
}

impl FusionCandidate for DocCandidateRow {
    fn dense_dist(&self) -> f64 {
        // DocCandidateRow no longer stores dist; fuse_rrf is dead code after the
        // SQL-RRF rewrite. Return a stub so the trait impl compiles.
        0.0
    }
    fn sparse(&self) -> (&[i64], &[f32]) {
        // DocCandidateRow no longer stores sparse vectors; stub for dead-code impl.
        (&[], &[])
    }
}

impl FusionCandidate for MemoryCandidateRow {
    fn dense_dist(&self) -> f64 {
        self.dist
    }
    fn sparse(&self) -> (&[i64], &[f32]) {
        (&self.sparse_idx, &self.sparse_val)
    }
}

/// Sparse dot-product of a query (presence-only, prebuilt index set) against a
/// candidate's stored (sparse_idx as i64, sparse_val) vector. Query indices are
/// u32 (FNV-1a hashes); candidate indices were stored as i64 (u32 cast up).
fn sparse_dot_with_set(qset: &HashSet<i64>, cand_idx: &[i64], cand_val: &[f32]) -> f32 {
    if qset.is_empty() || cand_idx.is_empty() {
        return 0.0;
    }
    let mut score = 0.0f32;
    for (idx, val) in cand_idx.iter().zip(cand_val.iter()) {
        if qset.contains(idx) {
            score += *val;
        }
    }
    score
}

/// Reciprocal Rank Fusion over dense-distance rank (asc) and sparse-score rank
/// (desc), `1/(60+rank)`, k=60. Returns candidate indices ordered by fused score,
/// truncated to `take`.
fn fuse_rrf<C: FusionCandidate>(candidates: &[C], query_idx: &[u32], take: usize) -> Vec<usize> {
    const K: f64 = 60.0;
    let n = candidates.len();

    // Dense rank: ascending distance (smaller = better).
    let mut dense_order: Vec<usize> = (0..n).collect();
    dense_order.sort_by(|&a, &b| {
        candidates[a]
            .dense_dist()
            .partial_cmp(&candidates[b].dense_dist())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Sparse rank: descending score (larger = better). Build the query index set
    // once and reuse it across all candidates.
    let qset: HashSet<i64> = query_idx.iter().map(|&i| i as i64).collect();
    let sparse_scores: Vec<f32> = candidates
        .iter()
        .map(|c| {
            let (idx, val) = c.sparse();
            sparse_dot_with_set(&qset, idx, val)
        })
        .collect();
    let mut sparse_order: Vec<usize> = (0..n).collect();
    sparse_order.sort_by(|&a, &b| {
        sparse_scores[b]
            .partial_cmp(&sparse_scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut fused = vec![0.0f64; n];
    for (rank, &i) in dense_order.iter().enumerate() {
        fused[i] += 1.0 / (K + rank as f64);
    }
    for (rank, &i) in sparse_order.iter().enumerate() {
        fused[i] += 1.0 / (K + rank as f64);
    }

    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        fused[b]
            .partial_cmp(&fused[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    order.truncate(take);
    order
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
