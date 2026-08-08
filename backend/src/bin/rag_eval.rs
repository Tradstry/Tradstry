//! Read-only RAG evaluation harness.
//!
//! Fixture mode:
//!   cargo run --bin rag_eval -- rag_eval_fixtures.json
//!
//! Privacy-safe live mode (uses existing indexed documents, performs no writes,
//! and never prints workspace/user/source IDs or document text):
//!   cargo run --bin rag_eval -- --auto 8
//!
//! Explicit local-dev schema preparation (refuses remote/prod targets):
//!   cargo run --bin rag_eval -- --prepare-local

use std::time::Instant;

use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;
use sqlx::{Connection, Executor, PgConnection};
use tradstry_backend::service::ai::client::AgentsClient;
use tradstry_backend::service::ai::vector_database::client::VectorDatabaseClient;

const DEFAULT_AUTO_CASES: usize = 8;
const DEFAULT_MIN_HIT_RATE_AT_5: f64 = 0.80;
const DEFAULT_MIN_MRR: f64 = 0.60;
const MAX_SOURCE_CHARS: usize = 8_000;

#[derive(Deserialize)]
struct Fixtures {
    user_id: String,
    #[serde(alias = "account_id")]
    workspace_id: String,
    min_hit_rate_at_5: Option<f64>,
    min_mrr: Option<f64>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    query: String,
    expected_source_ids: Vec<String>,
    #[serde(skip)]
    source_type: Option<String>,
}

#[derive(sqlx::FromRow)]
struct IndexedSource {
    user_id: String,
    workspace_id: String,
    source_type: String,
    source_id: String,
    title: String,
    content: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.first().is_some_and(|arg| arg == "--prepare-local") {
        prepare_local_schema().await?;
        return Ok(());
    }

    let (fixtures, privacy_safe_output) = if args.first().is_some_and(|arg| arg == "--auto") {
        let count = args
            .get(1)
            .map(|value| value.parse::<usize>())
            .transpose()
            .context("--auto case count must be a positive integer")?
            .unwrap_or(DEFAULT_AUTO_CASES);
        if count == 0 || count > 25 {
            bail!("--auto case count must be between 1 and 25");
        }
        (build_auto_fixtures(count).await?, true)
    } else {
        let path = args
            .first()
            .cloned()
            .unwrap_or_else(|| "rag_eval_fixtures.json".into());
        let raw = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
        let fixtures: Fixtures = serde_json::from_str(&raw).context("parse fixtures")?;
        reject_placeholders(&fixtures)?;
        (fixtures, false)
    };

    run_eval(&fixtures, privacy_safe_output).await
}

async fn prepare_local_schema() -> Result<()> {
    let postgres_url = std::env::var("POSTGRES_URL").context("POSTGRES_URL is not configured")?;
    let connect_options: sqlx::postgres::PgConnectOptions = postgres_url
        .parse()
        .context("failed to parse POSTGRES_URL")?;
    let host = connect_options.get_host();
    ensure_local_host(host)?;
    let environment = std::env::var("POSTGRES_DATABASE").unwrap_or_default();
    if !matches!(environment.trim(), "dev" | "test" | "local") {
        bail!(
            "--prepare-local requires POSTGRES_DATABASE=dev, test, or local; refusing {:?}",
            environment.trim()
        );
    }

    let client = VectorDatabaseClient::from_env()?;
    client
        .ensure_schema()
        .await
        .context("local vector schema preparation failed")?;
    println!("local {environment} vector schema is current");
    Ok(())
}

fn ensure_local_host(host: &str) -> Result<()> {
    if matches!(host, "localhost" | "127.0.0.1" | "::1") {
        return Ok(());
    }
    bail!("--prepare-local refuses non-loopback database hosts")
}

fn reject_placeholders(fixtures: &Fixtures) -> Result<()> {
    let has_placeholder = fixtures.user_id.contains("REPLACE_")
        || fixtures.workspace_id.contains("REPLACE_")
        || fixtures.cases.iter().any(|case| {
            case.expected_source_ids
                .iter()
                .any(|id| id.contains("REPLACE_"))
        });
    if has_placeholder {
        bail!("fixture contains placeholder IDs; provide real safe fixture values or use --auto");
    }
    if fixtures.cases.is_empty() {
        bail!("fixture must contain at least one case");
    }
    Ok(())
}

async fn run_eval(fixtures: &Fixtures, privacy_safe_output: bool) -> Result<()> {
    let client = VectorDatabaseClient::from_env_read_only()?;
    client.health_check().await?;
    let ks = [1usize, 3, 5, 10];
    let mut hits = [0u32; 4];
    let mut rr_sum = 0.0f64;
    let mut latencies_ms = Vec::with_capacity(fixtures.cases.len());

    for (index, case) in fixtures.cases.iter().enumerate() {
        let started = Instant::now();
        let results = client
            .hybrid_search(
                &case.query,
                &fixtures.user_id,
                &fixtures.workspace_id,
                None,
                None,
                10,
            )
            .await?;
        let latency_ms = started.elapsed().as_secs_f64() * 1_000.0;
        latencies_ms.push(latency_ms);
        let ranked: Vec<&str> = results
            .iter()
            .map(|result| result.source_id.as_str())
            .collect();
        let first_hit = ranked.iter().position(|id| {
            case.expected_source_ids
                .iter()
                .any(|expected| expected == id)
        });
        if let Some(position) = first_hit {
            rr_sum += 1.0 / (position as f64 + 1.0);
            for (metric_index, k) in ks.iter().enumerate() {
                if position < *k {
                    hits[metric_index] += 1;
                }
            }
        }
        if privacy_safe_output {
            println!(
                "case {:02} type={:<16} first_hit_rank={:?} latency_ms={:.0}",
                index + 1,
                case.source_type.as_deref().unwrap_or("unknown"),
                first_hit.map(|position| position + 1),
                latency_ms
            );
        } else {
            println!(
                "Q: {:<40} first_hit_rank={:?} latency_ms={:.0}",
                case.query,
                first_hit.map(|position| position + 1),
                latency_ms
            );
        }
    }

    let n = fixtures.cases.len() as f64;
    let hit_rate_at_5 = hits[2] as f64 / n;
    let mrr = rr_sum / n;
    latencies_ms.sort_by(f64::total_cmp);
    let p50 = percentile(&latencies_ms, 0.50);
    let p95 = percentile(&latencies_ms, 0.95);

    println!("\n=== RAG eval ({} cases) ===", fixtures.cases.len());
    for (index, k) in ks.iter().enumerate() {
        println!("hit-rate@{}: {:.1}%", k, 100.0 * hits[index] as f64 / n);
    }
    println!("MRR: {mrr:.3}");
    println!("latency p50/p95: {p50:.0}/{p95:.0} ms");

    let min_hit_rate_at_5 = fixtures
        .min_hit_rate_at_5
        .unwrap_or(DEFAULT_MIN_HIT_RATE_AT_5);
    let min_mrr = fixtures.min_mrr.unwrap_or(DEFAULT_MIN_MRR);
    if hit_rate_at_5 < min_hit_rate_at_5 {
        bail!(
            "RAG eval failed: hit-rate@5 {hit_rate_at_5:.3} is below required {min_hit_rate_at_5:.3}"
        );
    }
    if mrr < min_mrr {
        bail!("RAG eval failed: MRR {mrr:.3} is below required {min_mrr:.3}");
    }
    Ok(())
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let index = ((sorted.len() - 1) as f64 * percentile).ceil() as usize;
    sorted[index]
}

async fn build_auto_fixtures(count: usize) -> Result<Fixtures> {
    let mut connection = read_only_connection().await?;
    let sources = discover_sources(&mut connection, count).await?;
    let first = sources
        .first()
        .ok_or_else(|| anyhow!("no indexed RAG sources were found in the configured schema"))?;
    let user_id = first.user_id.clone();
    let workspace_id = first.workspace_id.clone();
    let agents = AgentsClient::from_env()?;
    println!(
        "building {} privacy-safe cases from an existing indexed workspace (identifiers redacted)",
        sources.len()
    );

    let mut cases = Vec::with_capacity(sources.len());
    for source in sources {
        let query = generate_retrieval_query(&agents, &source).await?;
        cases.push(Case {
            query,
            expected_source_ids: vec![source.source_id],
            source_type: Some(source.source_type),
        });
    }
    connection.close().await?;

    Ok(Fixtures {
        user_id,
        workspace_id,
        min_hit_rate_at_5: Some(DEFAULT_MIN_HIT_RATE_AT_5),
        min_mrr: Some(DEFAULT_MIN_MRR),
        cases,
    })
}

async fn read_only_connection() -> Result<PgConnection> {
    let postgres_url = std::env::var("POSTGRES_URL").context("POSTGRES_URL is not configured")?;
    let connect_options: sqlx::postgres::PgConnectOptions = postgres_url
        .parse()
        .context("failed to parse POSTGRES_URL")?;
    let search_path = tradstry_backend::service::db::config::search_path()?;

    let mut connection = PgConnection::connect_with(&connect_options)
        .await
        .context("failed to connect to the evaluation database")?;
    if let Some(search_path) = search_path {
        connection
            .execute(sqlx::AssertSqlSafe(format!(
                "SET search_path TO {search_path}"
            )))
            .await
            .context("failed to select the configured database schema")?;
    }
    connection
        .execute("SET default_transaction_read_only = on")
        .await
        .context("failed to enable database read-only mode")?;
    connection
        .execute("SET client_min_messages = WARNING")
        .await?;
    Ok(connection)
}

async fn discover_sources(
    connection: &mut PgConnection,
    count: usize,
) -> Result<Vec<IndexedSource>> {
    let table: Option<String> = sqlx::query_scalar("SELECT to_regclass('vector_documents')::text")
        .fetch_one(&mut *connection)
        .await
        .context("failed to inspect the RAG schema")?;
    if table.is_none() {
        bail!("vector_documents does not exist in the configured schema");
    }

    let rows = sqlx::query_as::<_, IndexedSource>(
        r#"
        WITH ranked_chunks AS (
            SELECT user_id, workspace_id, source_type, source_id, title, content, created_at, id,
                   ROW_NUMBER() OVER (
                       PARTITION BY user_id, workspace_id, source_type, source_id
                       ORDER BY char_length(content) DESC, created_at DESC, id
                   ) AS chunk_rank
            FROM vector_documents
            WHERE char_length(trim(content)) >= 80
        ),
        sources AS (
            SELECT user_id, workspace_id, source_type, source_id, MAX(title) AS title,
                   string_agg(content, E'\n\n' ORDER BY chunk_rank) AS content
            FROM ranked_chunks
            WHERE chunk_rank <= 3
            GROUP BY user_id, workspace_id, source_type, source_id
        ),
        chosen_workspace AS (
            SELECT user_id, workspace_id, COUNT(*) AS source_count
            FROM sources
            GROUP BY user_id, workspace_id
            ORDER BY source_count DESC
            LIMIT 1
        ),
        diverse AS (
            SELECT sources.*, ROW_NUMBER() OVER (
                PARTITION BY sources.source_type ORDER BY md5(sources.source_id)
            ) AS type_rank
            FROM sources
            JOIN chosen_workspace USING (user_id, workspace_id)
        )
        SELECT user_id, workspace_id, source_type, source_id, title, content
        FROM diverse
        ORDER BY type_rank, source_type, md5(source_id)
        LIMIT $1
        "#,
    )
    .bind(count as i64)
    .fetch_all(&mut *connection)
    .await
    .context("failed to discover indexed RAG evaluation sources")?;

    if rows.len() < count {
        println!(
            "requested {count} cases; the largest indexed workspace has {} usable distinct sources",
            rows.len()
        );
    }
    Ok(rows)
}

async fn generate_retrieval_query(agents: &AgentsClient, source: &IndexedSource) -> Result<String> {
    let content: String = source.content.chars().take(MAX_SOURCE_CHARS).collect();
    let prompt = format!(
        "Document type: {}\nDocument title: {}\nDocument content:\n{}",
        source.source_type, source.title, content
    );
    let raw = agents
        .prompt_with(
            "Write one realistic question a trader would ask about this private document. The question must depend on a distinctive factual detail in the content, but must not mention the document title, IDs, metadata, or that a document exists. Output only the question, on one line, with no quotation marks or explanation.",
            128,
            &prompt,
        )
        .await
        .context("Gemini failed to generate a RAG evaluation query")?;
    let query = raw
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().trim_matches(['\'', '"']).to_owned())
        .unwrap_or_default();
    if query.len() < 12 || query.len() > 500 {
        bail!("Gemini generated an invalid RAG evaluation query length");
    }
    Ok(query)
}
