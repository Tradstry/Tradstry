//! Offline RAG eval harness. Runs hybrid_search per fixture query and reports
//! hit-rate@k and MRR against expected source_ids. Requires POSTGRES_URL +
//! VOYAGE_API_KEY in env. Usage: cargo run --bin rag_eval -- rag_eval_fixtures.json
use anyhow::{Context, Result};
use serde::Deserialize;
use tradstry_backend::service::ai::vector_database::client::VectorDatabaseClient;

#[derive(Deserialize)]
struct Fixtures {
    user_id: String,
    workspace_id: String,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    query: String,
    expected_source_ids: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "rag_eval_fixtures.json".into());
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {path}"))?;
    let fx: Fixtures = serde_json::from_str(&raw).context("parse fixtures")?;
    let client = VectorDatabaseClient::from_env()?;

    let ks = [1usize, 3, 5, 10];
    let mut hits = [0u32; 4];
    let mut rr_sum = 0.0f64;

    for case in &fx.cases {
        let results = client
            .hybrid_search(&case.query, &fx.user_id, &fx.workspace_id, None, None, 10)
            .await?;
        let ranked: Vec<&str> = results.iter().map(|r| r.source_id.as_str()).collect();
        let first_hit = ranked
            .iter()
            .position(|id| case.expected_source_ids.iter().any(|e| e == id));
        if let Some(pos) = first_hit {
            rr_sum += 1.0 / (pos as f64 + 1.0);
            for (i, k) in ks.iter().enumerate() {
                if pos < *k {
                    hits[i] += 1;
                }
            }
        }
        println!(
            "Q: {:<40} first_hit_rank={:?}",
            case.query,
            first_hit.map(|p| p + 1)
        );
    }

    let n = fx.cases.len().max(1) as f64;
    println!("\n=== RAG eval ({} cases) ===", fx.cases.len());
    for (i, k) in ks.iter().enumerate() {
        println!("hit-rate@{}: {:.1}%", k, 100.0 * hits[i] as f64 / n);
    }
    println!("MRR: {:.3}", rr_sum / n);
    Ok(())
}
