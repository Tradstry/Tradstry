use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use log::{error, info};
use qdrant_client::qdrant::{
    Condition, DeletePointsBuilder, Filter, NamedVectors, PointStruct, Query, QueryPointsBuilder,
    UpsertPointsBuilder, Value as QdrantValue, Vector,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use uuid::Uuid;

use crate::service::{
    agents::{client::AgentsClient, vector_database::client::VectorDatabaseClient},
    read_service::{
        analytics::{self, AnalyticsTimeFilter, JournalAnalytics},
        journal, notebook, playbook,
    },
    turso::TursoClient,
};

use super::{
    db,
    types::{
        ARTIFACT_AI_INSIGHTS, ARTIFACT_AI_REPORT, ARTIFACT_MINDSET_SUMMARY, AiArtifactEnvelope,
        AiEventBus, AiEventEnvelope, AiJobRecord, AiRange, AiSourceDocument, AiTimeFilter,
        InsightBundle, InsightCard, JOB_GENERATE_AI_INSIGHTS, JOB_GENERATE_AI_REPORT,
        JOB_GENERATE_MINDSET_SUMMARY, JOB_REINDEX_ACCOUNT_SOURCES, MindsetSignal, MindsetSummary,
        ReportArtifact, ReportSection, SourceCitation,
    },
};

const DEFAULT_COLLECTION: &str = "tradstry_hybrid";
const POLL_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeneratedInsightCard {
    title: String,
    summary: String,
    category: String,
    severity: String,
    citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeneratedInsightBundle {
    overview: String,
    cards: Vec<GeneratedInsightCard>,
    next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeneratedReportSection {
    heading: String,
    body: String,
    citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeneratedReportArtifact {
    title: String,
    summary: String,
    sections: Vec<GeneratedReportSection>,
    next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeneratedMindsetSignal {
    pattern: String,
    evidence: String,
    coaching: String,
    citations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GeneratedMindsetSummary {
    overview: String,
    signals: Vec<GeneratedMindsetSignal>,
    routines: Vec<String>,
}

#[derive(Debug, Clone)]
struct RetrievedChunk {
    source_id: String,
    source_type: String,
    title: String,
    text: String,
}

pub async fn run_worker_loop(
    turso: std::sync::Arc<TursoClient>,
    agents: std::sync::Arc<AgentsClient>,
    vector_db: std::sync::Arc<VectorDatabaseClient>,
    events: AiEventBus,
) -> Result<()> {
    let lease_owner = format!("ai-worker-{}", Uuid::new_v4());

    info!(
        "[ai-worker] Worker started with lease_owner={}",
        lease_owner
    );
    loop {
        match db::lease_due_job(&turso, &lease_owner, 120).await {
            Ok(Some(job)) => {
                info!(
                    "[ai-worker] Leased job {} type={} for account={}",
                    job.id, job.job_type, job.account_id
                );
                let result = process_job(&turso, &agents, &vector_db, &events, &job).await;
                if let Err(error) = result {
                    error!("[ai-worker] Job {} failed: {:#}", job.id, error);
                    db::fail_job(&turso, &job.id, &error.to_string()).await?;
                    emit_event(
                        &events,
                        &job.user_id,
                        &job.id,
                        &job.account_id,
                        job.artifact_type.as_deref(),
                        "failed",
                        Some("AI generation failed"),
                        None,
                        Some(error.to_string()),
                    );
                } else {
                    info!("[ai-worker] Job {} completed successfully", job.id);
                    db::complete_job(&turso, &job.id).await?;
                }
            }
            Ok(None) => sleep(POLL_INTERVAL).await,
            Err(e) => {
                error!("[ai-worker] Error leasing job: {e}");
                sleep(POLL_INTERVAL).await;
            }
        }
    }
}

pub async fn enqueue_account_reindex(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    info!(
        "[ai-worker] Enqueuing reindex for user={} account={}",
        user_id, account_id
    );
    let _ = db::enqueue_job(
        turso,
        user_id,
        account_id,
        JOB_REINDEX_ACCOUNT_SOURCES,
        None,
        &AiTimeFilter::default(),
        &json!({}),
        Some(&format!("reindex:{user_id}:{account_id}")),
    )
    .await?;
    Ok(())
}

pub async fn enqueue_all_account_reindex(turso: &TursoClient, user_id: &str) -> Result<()> {
    let account_ids = db::list_user_account_ids(turso, user_id).await?;
    for account_id in account_ids {
        enqueue_account_reindex(turso, user_id, &account_id).await?;
    }
    Ok(())
}

async fn process_job(
    turso: &TursoClient,
    agents: &AgentsClient,
    vector_db: &VectorDatabaseClient,
    events: &AiEventBus,
    job: &AiJobRecord,
) -> Result<()> {
    let time_filter: AiTimeFilter =
        serde_json::from_str(&job.time_filter_json).unwrap_or_else(|_| AiTimeFilter::default());

    match job.job_type.as_str() {
        JOB_REINDEX_ACCOUNT_SOURCES => {
            emit_event(
                events,
                &job.user_id,
                &job.id,
                &job.account_id,
                None,
                "running",
                Some("Rebuilding account knowledge base"),
                None,
                None,
            );
            reindex_account_sources(turso, vector_db, &job.user_id, &job.account_id).await
        }
        JOB_GENERATE_AI_INSIGHTS => {
            generate_insights_job(turso, agents, vector_db, events, job, &time_filter).await
        }
        JOB_GENERATE_AI_REPORT => {
            generate_report_job(turso, agents, vector_db, events, job, &time_filter).await
        }
        JOB_GENERATE_MINDSET_SUMMARY => {
            generate_mindset_job(turso, agents, vector_db, events, job, &time_filter).await
        }
        other => Err(anyhow!("unknown ai job type: {other}")),
    }
}

async fn reindex_account_sources(
    turso: &TursoClient,
    vector_db: &VectorDatabaseClient,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    let user_db = turso.get_user_db(user_id).await?;
    let entries = journal::list_journal_entries(&user_db)
        .await?
        .into_iter()
        .filter(|entry| entry.account_id == account_id)
        .collect::<Vec<_>>();
    let notes = notebook::list_notebook_notes(&user_db, Some(account_id)).await?;
    let playbooks = playbook::list_playbooks(&user_db).await?;

    let mut docs = Vec::new();

    for entry in entries {
        let body = format!(
            "Trade on {symbol} ({symbol_name}). Status: {status}. Trade type: {trade_type}. Entry {entry_price:.2}, exit {exit_price:.2}, size {position_size:.2}. Total PL percentage {total_pl:.2}, ROI {net_roi:.2}, risk reward {risk_reward:.2}. Entry tactics: {entry_tactics}. Edges spotted: {edges_spotted}. Mistakes: {mistakes}. Notes: {notes}",
            symbol = entry.symbol,
            symbol_name = entry.symbol_name,
            status = entry.status,
            trade_type = entry.trade_type,
            entry_price = entry.entry_price,
            exit_price = entry.exit_price,
            position_size = entry.position_size,
            total_pl = entry.total_pl,
            net_roi = entry.net_roi,
            risk_reward = entry.risk_reward,
            entry_tactics = entry.entry_tactics,
            edges_spotted = entry.edges_spotted,
            mistakes = entry.mistakes,
            notes = entry.notes.clone().unwrap_or_default(),
        );
        docs.push(build_source_doc(
            user_id,
            account_id,
            "journal_entry",
            &entry.id,
            &format!("Trade review for {}", entry.symbol),
            &body,
            json!({
                "symbol": entry.symbol,
                "closeDate": entry.close_date,
                "playbookId": entry.playbook_id,
                "reviewed": entry.reviewed,
            }),
        ));
    }

    for note in notes {
        let body = extract_notebook_text(&note.document_json);
        if body.is_empty() {
            continue;
        }
        docs.push(build_source_doc(
            user_id,
            account_id,
            "notebook_note",
            &note.id,
            &note.title,
            &body,
            json!({
                "tradeIds": note.trade_ids,
                "imageCount": note.images.len(),
                "updatedAt": note.updated_at,
            }),
        ));
    }

    for book in playbooks {
        let body = format!(
            "Playbook {name}. Edge: {edge}. Entry rules: {entry_rules}. Exit rules: {exit_rules}. Position sizing rules: {position_sizing_rules}. Additional rules: {additional_rules}",
            name = book.name,
            edge = book.edge_name,
            entry_rules = book.entry_rules,
            exit_rules = book.exit_rules,
            position_sizing_rules = book.position_sizing_rules,
            additional_rules = book.additional_rules.unwrap_or_default(),
        );
        docs.push(build_source_doc(
            user_id,
            account_id,
            "playbook",
            &book.id,
            &book.name,
            &body,
            json!({
                "edgeName": book.edge_name,
                "tradeCount": book.trade_count,
            }),
        ));
    }

    db::replace_source_documents_for_account(turso, user_id, account_id, &docs).await?;
    reindex_vectors_for_account(vector_db, user_id, account_id, &docs).await?;
    Ok(())
}

async fn generate_insights_job(
    turso: &TursoClient,
    agents: &AgentsClient,
    vector_db: &VectorDatabaseClient,
    events: &AiEventBus,
    job: &AiJobRecord,
    time_filter: &AiTimeFilter,
) -> Result<()> {
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_AI_INSIGHTS),
        "retrieving_sources",
        Some("Collecting account evidence"),
        None,
        None,
    );
    let sources = retrieve_for_queries(
        vector_db,
        &job.user_id,
        &job.account_id,
        &[
            "recurring trading mistakes and discipline issues",
            "best setups and edges that keep working",
            "risk management problems and execution drift",
        ],
        8,
    )
    .await?;
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_AI_INSIGHTS),
        "generating",
        Some("Generating AI insights"),
        None,
        None,
    );

    let prompt =
        build_insights_prompt(turso, &job.user_id, &job.account_id, time_filter, &sources).await?;
    let raw = agents.prompt(prompt).await?;
    let parsed: GeneratedInsightBundle =
        parse_model_json(&raw).context("failed to parse generated insight bundle")?;
    let artifact = AiArtifactEnvelope {
        artifact_type: ARTIFACT_AI_INSIGHTS.to_string(),
        insight_bundle: Some(InsightBundle {
            overview: parsed.overview,
            cards: parsed
                .cards
                .into_iter()
                .map(|card| InsightCard {
                    title: card.title,
                    summary: card.summary,
                    category: card.category,
                    severity: card.severity,
                    citations: map_citations(&sources, &card.citations),
                })
                .collect(),
            next_actions: parsed.next_actions,
        }),
        report: None,
        mindset_summary: None,
    };
    let source_docs =
        db::list_source_documents_for_account(turso, &job.user_id, &job.account_id).await?;
    db::save_artifact(
        turso,
        &job.user_id,
        &job.account_id,
        ARTIFACT_AI_INSIGHTS,
        time_filter,
        "completed",
        agents.model(),
        "insights_v1",
        &artifact,
        &select_cited_docs(&source_docs, &sources),
    )
    .await?;
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_AI_INSIGHTS),
        "completed",
        Some("AI insights ready"),
        Some(artifact),
        None,
    );
    Ok(())
}

async fn generate_report_job(
    turso: &TursoClient,
    agents: &AgentsClient,
    vector_db: &VectorDatabaseClient,
    events: &AiEventBus,
    job: &AiJobRecord,
    time_filter: &AiTimeFilter,
) -> Result<()> {
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_AI_REPORT),
        "retrieving_sources",
        Some("Collecting report evidence"),
        None,
        None,
    );
    let sources = retrieve_for_queries(
        vector_db,
        &job.user_id,
        &job.account_id,
        &[
            "trading performance summary and best setups",
            "losses mistakes and what regressed",
            "execution discipline and next improvements",
        ],
        10,
    )
    .await?;
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_AI_REPORT),
        "generating",
        Some("Generating AI report"),
        None,
        None,
    );
    let prompt =
        build_report_prompt(turso, &job.user_id, &job.account_id, time_filter, &sources).await?;
    let raw = agents.prompt(prompt).await?;
    let parsed: GeneratedReportArtifact =
        parse_model_json(&raw).context("failed to parse generated report")?;
    let artifact = AiArtifactEnvelope {
        artifact_type: ARTIFACT_AI_REPORT.to_string(),
        insight_bundle: None,
        report: Some(ReportArtifact {
            title: parsed.title,
            summary: parsed.summary,
            sections: parsed
                .sections
                .into_iter()
                .map(|section| ReportSection {
                    heading: section.heading,
                    body: section.body,
                    citations: map_citations(&sources, &section.citations),
                })
                .collect(),
            next_actions: parsed.next_actions,
        }),
        mindset_summary: None,
    };
    let source_docs =
        db::list_source_documents_for_account(turso, &job.user_id, &job.account_id).await?;
    db::save_artifact(
        turso,
        &job.user_id,
        &job.account_id,
        ARTIFACT_AI_REPORT,
        time_filter,
        "completed",
        agents.model(),
        "report_v1",
        &artifact,
        &select_cited_docs(&source_docs, &sources),
    )
    .await?;
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_AI_REPORT),
        "completed",
        Some("AI report ready"),
        Some(artifact),
        None,
    );
    Ok(())
}

async fn generate_mindset_job(
    turso: &TursoClient,
    agents: &AgentsClient,
    vector_db: &VectorDatabaseClient,
    events: &AiEventBus,
    job: &AiJobRecord,
    time_filter: &AiTimeFilter,
) -> Result<()> {
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_MINDSET_SUMMARY),
        "retrieving_sources",
        Some("Collecting mindset evidence"),
        None,
        None,
    );
    let sources = retrieve_for_queries(
        vector_db,
        &job.user_id,
        &job.account_id,
        &[
            "discipline confidence hesitation overtrading revenge trading",
            "mistakes emotional patterns and routines",
            "psychology and process consistency in trading notes",
        ],
        10,
    )
    .await?;
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_MINDSET_SUMMARY),
        "generating",
        Some("Generating mindset summary"),
        None,
        None,
    );
    let prompt =
        build_mindset_prompt(turso, &job.user_id, &job.account_id, time_filter, &sources).await?;
    let raw = agents.prompt(prompt).await?;
    let parsed: GeneratedMindsetSummary =
        parse_model_json(&raw).context("failed to parse generated mindset summary")?;
    let artifact = AiArtifactEnvelope {
        artifact_type: ARTIFACT_MINDSET_SUMMARY.to_string(),
        insight_bundle: None,
        report: None,
        mindset_summary: Some(MindsetSummary {
            overview: parsed.overview,
            signals: parsed
                .signals
                .into_iter()
                .map(|signal| MindsetSignal {
                    pattern: signal.pattern,
                    evidence: signal.evidence,
                    coaching: signal.coaching,
                    citations: map_citations(&sources, &signal.citations),
                })
                .collect(),
            routines: parsed.routines,
        }),
    };
    let source_docs =
        db::list_source_documents_for_account(turso, &job.user_id, &job.account_id).await?;
    db::save_artifact(
        turso,
        &job.user_id,
        &job.account_id,
        ARTIFACT_MINDSET_SUMMARY,
        time_filter,
        "completed",
        agents.model(),
        "mindset_v1",
        &artifact,
        &select_cited_docs(&source_docs, &sources),
    )
    .await?;
    emit_event(
        events,
        &job.user_id,
        &job.id,
        &job.account_id,
        Some(ARTIFACT_MINDSET_SUMMARY),
        "completed",
        Some("Mindset summary ready"),
        Some(artifact),
        None,
    );
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn emit_event(
    events: &AiEventBus,
    user_id: &str,
    job_id: &str,
    account_id: &str,
    artifact_type: Option<&str>,
    status: &str,
    message: Option<&str>,
    artifact: Option<AiArtifactEnvelope>,
    error: Option<String>,
) {
    let _ = events.send(AiEventEnvelope {
        user_id: user_id.to_string(),
        job_id: job_id.to_string(),
        account_id: account_id.to_string(),
        artifact_type: artifact_type.map(str::to_string),
        status: status.to_string(),
        message: message.map(str::to_string),
        artifact,
        error,
    });
}

fn build_source_doc(
    user_id: &str,
    account_id: &str,
    source_type: &str,
    source_id: &str,
    title: &str,
    body_text: &str,
    metadata: Value,
) -> AiSourceDocument {
    let metadata_json = metadata.to_string();
    let content_hash = format!(
        "{:x}",
        md5::compute(format!("{title}|{body_text}|{metadata_json}"))
    );
    AiSourceDocument {
        id: Uuid::new_v4().to_string(),
        user_id: user_id.to_string(),
        account_id: account_id.to_string(),
        source_type: source_type.to_string(),
        source_id: source_id.to_string(),
        title: title.to_string(),
        body_text: body_text.to_string(),
        metadata_json,
        content_hash,
    }
}

fn extract_notebook_text(document_json: &str) -> String {
    fn walk(node: &Value, output: &mut String) {
        if let Some(text) = node.get("text").and_then(Value::as_str) {
            output.push_str(text);
            output.push(' ');
        }
        if let Some(children) = node.get("children").and_then(Value::as_array) {
            for child in children {
                walk(child, output);
            }
        }
    }

    match serde_json::from_str::<Value>(document_json) {
        Ok(parsed) => {
            let mut text = String::new();
            walk(&parsed, &mut text);
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        }
        Err(_) => String::new(),
    }
}

async fn reindex_vectors_for_account(
    vector_db: &VectorDatabaseClient,
    user_id: &str,
    account_id: &str,
    docs: &[AiSourceDocument],
) -> Result<()> {
    let mut chunks = Vec::new();
    for doc in docs {
        for (chunk_index, text) in chunk_text(&doc.body_text).into_iter().enumerate() {
            chunks.push((doc.clone(), chunk_index, text));
        }
    }

    delete_account_vectors(vector_db, user_id, account_id).await?;
    if chunks.is_empty() {
        return Ok(());
    }

    let texts = chunks
        .iter()
        .map(|(_, _, text)| text.clone())
        .collect::<Vec<_>>();
    let embeddings = vector_db.embed_texts(texts, Some("document")).await?;
    vector_db.ensure_hybrid_collection().await?;

    let points = chunks
        .into_iter()
        .zip(embeddings)
        .map(|((doc, chunk_index, text), embedding)| {
            let payload: HashMap<String, QdrantValue> = [
                (
                    "user_id".to_string(),
                    QdrantValue::from(user_id.to_string()),
                ),
                (
                    "account_id".to_string(),
                    QdrantValue::from(account_id.to_string()),
                ),
                ("source_id".to_string(), QdrantValue::from(doc.source_id)),
                (
                    "source_type".to_string(),
                    QdrantValue::from(doc.source_type),
                ),
                ("title".to_string(), QdrantValue::from(doc.title)),
                ("text".to_string(), QdrantValue::from(text.clone())),
                (
                    "chunk_index".to_string(),
                    QdrantValue::from(chunk_index as i64),
                ),
            ]
            .into();

            // Build both dense and sparse vectors for hybrid search
            let (sparse_indices, sparse_values) =
                crate::service::agents::vector_database::sparse::text_to_sparse_vector(&text);
            let named_vectors = NamedVectors::default()
                .add_vector("dense", Vector::new_dense(embedding))
                .add_vector("sparse", Vector::new_sparse(sparse_indices, sparse_values));

            PointStruct::new(Uuid::new_v4().to_string(), named_vectors, payload)
        })
        .collect::<Vec<_>>();

    vector_db
        .qdrant()
        .upsert_points(UpsertPointsBuilder::new(collection_name(vector_db), points))
        .await
        .context("failed to upsert ai source vectors")?;

    Ok(())
}

async fn delete_account_vectors(
    vector_db: &VectorDatabaseClient,
    user_id: &str,
    account_id: &str,
) -> Result<()> {
    let filter = Filter::must([
        Condition::matches("user_id", user_id.to_string()),
        Condition::matches("account_id", account_id.to_string()),
    ]);

    let _ = vector_db
        .qdrant()
        .delete_points(
            DeletePointsBuilder::new(collection_name(vector_db))
                .points(filter)
                .wait(true),
        )
        .await;

    Ok(())
}

async fn retrieve_for_queries(
    vector_db: &VectorDatabaseClient,
    user_id: &str,
    account_id: &str,
    queries: &[&str],
    per_query_limit: usize,
) -> Result<Vec<(String, RetrievedChunk)>> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for query in queries {
        log::debug!("retrieving qdrant results for query: {:?}", query);
        let vector = vector_db
            .embed_text(*query, Some("query"))
            .await
            .map_err(|e| {
                log::error!("embedding failed for query {:?}: {:?}", query, e);
                e
            })?;
        let filter = Filter::must([
            Condition::matches("user_id", user_id.to_string()),
            Condition::matches("account_id", account_id.to_string()),
        ]);
        let response = vector_db
            .qdrant()
            .query(
                QueryPointsBuilder::new(collection_name(vector_db))
                    .query(Query::new_nearest(vector))
                    .using("dense")
                    .filter(filter)
                    .limit(per_query_limit as u64)
                    .with_payload(true),
            )
            .await
            .map_err(|e| {
                log::error!(
                    "qdrant query failed for user_id={}, account_id={}, query={:?}: {:?}",
                    user_id,
                    account_id,
                    query,
                    e
                );
                e
            })
            .context("failed to query qdrant for ai retrieval")?;

        for point in response.result {
            let payload = point.payload;
            let source_id = payload
                .get("source_id")
                .and_then(|value| value.as_str())
                .cloned()
                .unwrap_or_default();
            let title = payload
                .get("title")
                .and_then(|value| value.as_str())
                .cloned()
                .unwrap_or_default();
            let text = payload
                .get("text")
                .and_then(|value| value.as_str())
                .cloned()
                .unwrap_or_default();
            let source_type = payload
                .get("source_type")
                .and_then(|value| value.as_str())
                .cloned()
                .unwrap_or_default();
            if text.is_empty() {
                continue;
            }
            let key = format!("{source_type}:{source_id}:{title}");
            if seen.insert(key.clone()) {
                results.push((
                    format!("SRC-{}", seen.len()),
                    RetrievedChunk {
                        source_id,
                        source_type,
                        title,
                        text,
                    },
                ));
            }
        }
    }

    let reranker_model = &vector_db.config().voyage.reranker_model;
    info!("reranking ai sources with {}", reranker_model);
    let reranked = vector_db
        .rerank(
            queries.join(" "),
            results
                .iter()
                .map(|(_, chunk)| chunk.text.clone())
                .collect(),
            Some(8),
        )
        .await?;
    let ordered = reranked
        .into_iter()
        .filter_map(|result| results.get(result.index).cloned())
        .collect::<Vec<_>>();
    Ok(ordered)
}

fn collection_name(vector_db: &VectorDatabaseClient) -> &str {
    vector_db
        .config()
        .qdrant
        .collection
        .as_deref()
        .unwrap_or(DEFAULT_COLLECTION)
}

fn chunk_text(text: &str) -> Vec<String> {
    let words = text.split_whitespace().collect::<Vec<_>>();
    if words.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0usize;
    while start < words.len() {
        let end = (start + 120).min(words.len());
        chunks.push(words[start..end].join(" "));
        if end == words.len() {
            break;
        }
        start = end.saturating_sub(20);
    }
    chunks
}

fn map_citations(sources: &[(String, RetrievedChunk)], refs: &[String]) -> Vec<SourceCitation> {
    refs.iter()
        .filter_map(|reference| {
            let (_, chunk) = sources.iter().find(|(id, _)| id == reference)?;
            Some(SourceCitation {
                source_type: chunk.source_type.clone(),
                source_id: chunk.source_id.clone(),
                title: chunk.title.clone(),
                excerpt: chunk.text.chars().take(180).collect(),
            })
        })
        .collect()
}

fn select_cited_docs(
    docs: &[AiSourceDocument],
    sources: &[(String, RetrievedChunk)],
) -> Vec<AiSourceDocument> {
    let source_ids = sources
        .iter()
        .map(|(_, chunk)| (&chunk.source_type, &chunk.source_id))
        .collect::<HashSet<_>>();
    docs.iter()
        .filter(|doc| source_ids.contains(&(&doc.source_type, &doc.source_id)))
        .cloned()
        .collect()
}

async fn build_insights_prompt(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    time_filter: &AiTimeFilter,
    sources: &[(String, RetrievedChunk)],
) -> Result<String> {
    let analytics = analytics_snapshot(turso, user_id, account_id, time_filter).await?;
    Ok(format!(
        "You are a trading journal intelligence system. Generate grounded insights only from the provided sources and analytics.\n\
         Rules:\n\
         - Never give financial advice.\n\
         - Never invent facts.\n\
         - Every card must cite 1-3 source refs from the list.\n\
         - Return JSON only.\n\
         JSON schema:\n\
         {{\"overview\": string, \"cards\": [{{\"title\": string, \"summary\": string, \"category\": string, \"severity\": \"low\"|\"medium\"|\"high\", \"citations\": [\"SRC-1\"]}}], \"next_actions\": [string]}}\n\n\
         Analytics:\n{analytics}\n\n\
         Sources:\n{sources_text}",
        analytics = analytics,
        sources_text = format_sources(sources)
    ))
}

async fn build_report_prompt(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    time_filter: &AiTimeFilter,
    sources: &[(String, RetrievedChunk)],
) -> Result<String> {
    let analytics = analytics_snapshot(turso, user_id, account_id, time_filter).await?;
    Ok(format!(
        "You are a trading performance reporting system. Build a grounded account review from the provided analytics and source excerpts.\n\
         Rules:\n\
         - Never invent performance claims.\n\
         - Never give financial advice or predictions.\n\
         - Every section must cite 1-3 source refs.\n\
         - Return JSON only.\n\
         JSON schema:\n\
         {{\"title\": string, \"summary\": string, \"sections\": [{{\"heading\": string, \"body\": string, \"citations\": [\"SRC-1\"]}}], \"next_actions\": [string]}}\n\n\
         Analytics:\n{analytics}\n\n\
         Sources:\n{sources_text}",
        analytics = analytics,
        sources_text = format_sources(sources)
    ))
}

async fn build_mindset_prompt(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    time_filter: &AiTimeFilter,
    sources: &[(String, RetrievedChunk)],
) -> Result<String> {
    let analytics = analytics_snapshot(turso, user_id, account_id, time_filter).await?;
    Ok(format!(
        "You are a trading mindset analyst. Infer discipline and psychology patterns only from explicit notes, mistakes, tactics, and notebook content.\n\
         Rules:\n\
         - Never diagnose mental health conditions.\n\
         - Never invent emotions that are not evidenced.\n\
         - Every signal must cite 1-3 source refs.\n\
         - Return JSON only.\n\
         JSON schema:\n\
         {{\"overview\": string, \"signals\": [{{\"pattern\": string, \"evidence\": string, \"coaching\": string, \"citations\": [\"SRC-1\"]}}], \"routines\": [string]}}\n\n\
         Analytics:\n{analytics}\n\n\
         Sources:\n{sources_text}",
        analytics = analytics,
        sources_text = format_sources(sources)
    ))
}

fn format_sources(sources: &[(String, RetrievedChunk)]) -> String {
    sources
        .iter()
        .map(|(reference, chunk)| {
            format!(
                "[{reference}] type={source_type} title={title}\n{body}",
                source_type = chunk.source_type,
                title = chunk.title,
                body = chunk.text
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

async fn analytics_snapshot(
    turso: &TursoClient,
    user_id: &str,
    account_id: &str,
    time_filter: &AiTimeFilter,
) -> Result<String> {
    let user_db = turso.get_user_db(user_id).await?;
    let analytics_filter = to_analytics_time_filter(time_filter);
    let snapshot: JournalAnalytics =
        analytics::get_journal_analytics(&user_db, account_id, &analytics_filter).await?;
    serde_json::to_string_pretty(&snapshot).context("failed to serialize analytics snapshot")
}

fn to_analytics_time_filter(value: &AiTimeFilter) -> AnalyticsTimeFilter {
    match value.range {
        AiRange::Last7Days => AnalyticsTimeFilter::Last7Days,
        AiRange::Last30Days => AnalyticsTimeFilter::Last30Days,
        AiRange::YearToDate => AnalyticsTimeFilter::YearToDate,
        AiRange::Last1Year => AnalyticsTimeFilter::Last1Year,
        AiRange::Custom => AnalyticsTimeFilter::Custom {
            start_date: value.start_date.clone().unwrap_or_default(),
            end_date: value.end_date.clone().unwrap_or_default(),
        },
    }
}

fn parse_model_json<T>(raw: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let trimmed = raw.trim();
    let json = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();
    serde_json::from_str(json).context("model did not return valid JSON")
}
