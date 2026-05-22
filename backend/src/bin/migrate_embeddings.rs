// Re-embeds all Qdrant points with the configured Voyage model/dimension.
// One-off: run with `cargo run --bin migrate_embeddings`.
//
// SAFETY: This binary DELETES and recreates both Qdrant collections.
// Ensure VOYAGE_API_KEY has balance BEFORE running. A failed mid-loop
// re-embed leaves the collection partially filled.

use anyhow::{Context, Result};
use log::{info, warn};
use qdrant_client::qdrant::point_id::PointIdOptions;
use qdrant_client::qdrant::{PointId, ScrollPointsBuilder, Value};
use std::collections::HashMap;
use tradstry_backend::service::agents::vector_database::client::VectorDatabaseClient;

fn payload_str(p: &HashMap<String, Value>, key: &str) -> String {
    p.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
        .unwrap_or_default()
}

/// Scroll all points from a collection, returning (point_id_string, payload) pairs.
/// Uses the `next_page_offset` PointId cursor for pagination.
async fn scroll_all(
    client: &VectorDatabaseClient,
    collection: &str,
) -> Result<Vec<(String, HashMap<String, Value>)>> {
    let mut out: Vec<(String, HashMap<String, Value>)> = Vec::new();
    let mut offset: Option<PointId> = None;

    loop {
        let mut req = ScrollPointsBuilder::new(collection)
            .limit(128u32)
            .with_payload(true);

        if let Some(o) = offset.take() {
            req = req.offset(o);
        }

        let resp = client
            .qdrant()
            .scroll(req)
            .await
            .with_context(|| format!("scroll {collection} failed"))?;

        for pt in &resp.result {
            let id_str = pt
                .id
                .as_ref()
                .and_then(|i| i.point_id_options.as_ref())
                .map(|opt| match opt {
                    PointIdOptions::Uuid(u) => u.clone(),
                    PointIdOptions::Num(n) => n.to_string(),
                })
                .unwrap_or_default();

            out.push((id_str, pt.payload.clone()));
        }

        offset = resp.next_page_offset;
        if offset.is_none() {
            break;
        }
    }

    Ok(out)
}

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    // Load env from the file given as the first CLI arg (e.g. `.env.production`),
    // defaulting to `.env`. Shell-exported vars still take precedence.
    let env_file = std::env::args().nth(1).unwrap_or_else(|| ".env".to_string());
    if let Err(e) = dotenvy::from_filename(&env_file) {
        eprintln!("warning: could not load env file '{env_file}': {e}");
    }
    env_logger::init();
    info!("Loaded environment from {env_file}");

    let client = VectorDatabaseClient::from_env().context("Failed to build VectorDatabaseClient")?;

    // --- Safety check: confirm Voyage connectivity before any destructive operation ---
    info!("Verifying Voyage AI connectivity...");
    client
        .embed_text("connectivity check", Some("document"))
        .await
        .context(
            "Voyage connectivity check failed — ensure VOYAGE_API_KEY is set and has balance",
        )?;
    info!("Voyage connectivity verified.");

    // =========================================================
    // tradstry_hybrid
    // =========================================================
    info!("Scrolling all points from tradstry_hybrid...");
    let hybrid = scroll_all(&client, "tradstry_hybrid").await?;
    info!("Scrolled {} hybrid points.", hybrid.len());

    info!("Deleting tradstry_hybrid collection...");
    match client
        .qdrant()
        .delete_collection("tradstry_hybrid")
        .await
    {
        Ok(_) => info!("Deleted tradstry_hybrid."),
        Err(e) => warn!("Could not delete tradstry_hybrid (may not exist): {e}"),
    }

    info!("Recreating tradstry_hybrid at 2048 dims...");
    client
        .ensure_hybrid_collection()
        .await
        .context("ensure_hybrid_collection failed")?;

    info!("Re-embedding {} hybrid points...", hybrid.len());
    for (i, (id, p)) in hybrid.iter().enumerate() {
        let text = payload_str(p, "text");
        if text.is_empty() {
            warn!("Skipping hybrid point {id}: empty text payload");
            continue;
        }
        client
            .upsert_hybrid(
                id,
                &text,
                &payload_str(p, "user_id"),
                &payload_str(p, "account_id"),
                &payload_str(p, "source_type"),
                &payload_str(p, "source_id"),
                &payload_str(p, "created_at"),
            )
            .await
            .with_context(|| format!("re-upsert hybrid point {id} (index {i})"))?;

        if (i + 1) % 10 == 0 {
            info!("  ... {}/{} hybrid points done", i + 1, hybrid.len());
        }
    }
    info!("Re-embedded {} hybrid points.", hybrid.len());

    // =========================================================
    // tradstry_memories
    // =========================================================
    info!("Scrolling all points from tradstry_memories...");
    let memories = scroll_all(&client, "tradstry_memories").await?;
    info!("Scrolled {} memory points.", memories.len());

    info!("Deleting tradstry_memories collection...");
    match client
        .qdrant()
        .delete_collection("tradstry_memories")
        .await
    {
        Ok(_) => info!("Deleted tradstry_memories."),
        Err(e) => warn!("Could not delete tradstry_memories (may not exist): {e}"),
    }

    info!("Recreating tradstry_memories at 2048 dims...");
    client
        .ensure_memories_collection()
        .await
        .context("ensure_memories_collection failed")?;

    // Batch the re-embed: each Voyage request bundles many memories so we make a
    // handful of requests (kept under the TPM ceiling) instead of one per point.
    // New point ids are generated; memories are looked up by (user_id, memory_key).
    const MAX_BATCH_TOKENS: usize = 8000; // stay under the 10K TPM per-request ceiling
    const MAX_BATCH_ITEMS: usize = 128;

    let items: Vec<(String, String, String)> = memories
        .iter()
        .filter_map(|(id, p)| {
            let text = payload_str(p, "text");
            if text.is_empty() {
                warn!("Skipping memory point {id}: empty text payload");
                return None;
            }
            Some((payload_str(p, "user_id"), payload_str(p, "memory_key"), text))
        })
        .collect();
    let total = items.len();
    info!("Re-embedding {total} memory points (batched)...");

    let mut batch: Vec<(String, String, String)> = Vec::new();
    let mut batch_tokens = 0usize;
    let mut done = 0usize;
    for item in items {
        let est = (item.2.len() / 4).max(1);
        if !batch.is_empty()
            && (batch.len() >= MAX_BATCH_ITEMS || batch_tokens + est > MAX_BATCH_TOKENS)
        {
            client.upsert_memories_batch(&batch).await.context("re-upsert memory batch")?;
            done += batch.len();
            info!("  ... {done}/{total} memory points done");
            batch.clear();
            batch_tokens = 0;
        }
        batch_tokens += est;
        batch.push(item);
    }
    if !batch.is_empty() {
        client.upsert_memories_batch(&batch).await.context("re-upsert memory batch")?;
        done += batch.len();
        info!("  ... {done}/{total} memory points done");
    }
    info!("Re-embedded {done} memory points.");

    info!("Migration complete. Both collections are now at Voyage 2048-dim vectors.");
    Ok(())
}
