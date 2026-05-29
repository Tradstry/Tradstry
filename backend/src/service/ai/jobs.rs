use std::{
    collections::{HashMap, HashSet},
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use log::{error, info};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::time::sleep;
use uuid::Uuid;

use crate::service::{
    ai::{
        client::AgentsClient,
        vector_database::{
            blocks::{Block, extract_notebook_blocks},
            chunking::chunk_blocks,
            client::{VectorDatabaseClient, VectorDocumentUpsert, VectorParentUpsert},
            context::{DocMeta, compose_embedded_text, deterministic_header},
        },
    },
    read_service::{
        analytics::{self, AnalyticsTimeFilter, JournalAnalytics},
        journal, notebook, playbook,
        playbook::PlaybookWithStats,
    },
    turso::{
        TursoClient,
        schema::tables::{journal_table::JournalEntry, tags_table},
    },
};

use super::{
    context_llm::{ENABLE_LLM_CONTEXT, generate_context_blurbs},
    db,
    types::{
        ARTIFACT_AI_INSIGHTS, ARTIFACT_AI_REPORT, ARTIFACT_MINDSET_SUMMARY, AiArtifactEnvelope,
        AiEventBus, AiEventEnvelope, AiJobRecord, AiRange, AiSourceDocument, AiTimeFilter,
        InsightBundle, InsightCard, JOB_GENERATE_AI_INSIGHTS, JOB_GENERATE_AI_REPORT,
        JOB_GENERATE_MINDSET_SUMMARY, JOB_REINDEX_ACCOUNT_SOURCES, MindsetSignal, MindsetSummary,
        ReportArtifact, ReportSection, SourceCitation,
    },
};

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
            reindex_account_sources(turso, agents, vector_db, &job.user_id, &job.account_id).await
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
    agents: &AgentsClient,
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

    // Batch-load every entry's tags once (one query) so `journal_blocks` can emit
    // tag-derived `Field` blocks alongside any legacy freeform content.
    let entry_ids = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<Vec<_>>();
    let trade_tags = tags_table::tags_for_trades(user_db.conn(), &entry_ids).await?;

    // Each indexable source carries its flat `AiSourceDocument` (for the source
    // record + display), its structure-aware `Vec<Block>` (for the chunker), and a
    // `DocMeta` (for the deterministic context header).
    let mut indexable: Vec<(AiSourceDocument, Vec<Block>, DocMeta)> = Vec::new();

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
        let title = format!("Trade review for {}", entry.symbol);
        let doc = build_source_doc(
            user_id,
            account_id,
            "journal_entry",
            &entry.id,
            &title,
            &body,
            json!({
                "symbol": entry.symbol,
                "closeDate": entry.close_date,
                "playbookId": entry.playbook_id,
                "reviewed": entry.reviewed,
            }),
        );
        let entry_tags = trade_tags.get(&entry.id).map(Vec::as_slice).unwrap_or(&[]);
        let blocks = journal_blocks(&entry, entry_tags);
        let meta = DocMeta {
            source_type: "journal_entry".to_string(),
            title,
            date: (!entry.close_date.is_empty()).then(|| entry.close_date.clone()),
            symbol: Some(entry.symbol.clone()),
        };
        indexable.push((doc, blocks, meta));
    }

    for note in notes {
        let blocks = extract_notebook_blocks(&note.document_json);
        if blocks.is_empty() {
            continue;
        }
        let body = blocks
            .iter()
            .map(|b| b.text.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let doc = build_source_doc(
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
        );
        let meta = DocMeta {
            source_type: "notebook_note".to_string(),
            title: note.title.clone(),
            date: (!note.updated_at.is_empty()).then(|| note.updated_at.clone()),
            symbol: None,
        };
        indexable.push((doc, blocks, meta));
    }

    for book in playbooks {
        let body = format!(
            "Playbook {name}. Edge: {edge}. Entry rules: {entry_rules}. Exit rules: {exit_rules}. Position sizing rules: {position_sizing_rules}. Additional rules: {additional_rules}",
            name = book.name,
            edge = book.edge_name,
            entry_rules = book.entry_rules,
            exit_rules = book.exit_rules,
            position_sizing_rules = book.position_sizing_rules,
            additional_rules = book.additional_rules.clone().unwrap_or_default(),
        );
        let doc = build_source_doc(
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
        );
        let blocks = playbook_blocks(&book);
        let meta = DocMeta {
            source_type: "playbook".to_string(),
            title: book.name.clone(),
            date: None,
            symbol: None,
        };
        indexable.push((doc, blocks, meta));
    }

    let docs = indexable
        .iter()
        .map(|(doc, _, _)| doc.clone())
        .collect::<Vec<_>>();
    db::replace_source_documents_for_account(turso, user_id, account_id, &docs).await?;
    reindex_vectors_for_account(agents, vector_db, user_id, account_id, &indexable).await?;
    Ok(())
}

/// One `Field` block per non-empty journal-entry text field, feeding the chunker.
///
/// Tags are emitted as one block per category (label = category name, value =
/// comma-joined tag names). The legacy freeform `entry_tactics`/`edges_spotted`/
/// `mistakes` fields are still emitted when non-empty so OLD trades keep their
/// historical embedding content (dual-read coexistence).
fn journal_blocks(entry: &JournalEntry, tags: &[tags_table::TradeTag]) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut push = |name: &str, value: &str| {
        if !value.trim().is_empty() {
            blocks.push(Block::field(name, value));
        }
    };
    push("symbol", &entry.symbol);
    push("symbol_name", &entry.symbol_name);
    push("status", &entry.status);
    push("trade_type", &entry.trade_type);

    // Group tags by category (preserving the batch query's sort order: category
    // sort_order/name, then tag name) into one Field block per category.
    let mut grouped: Vec<(String, Vec<String>)> = Vec::new();
    for trade_tag in tags {
        match grouped
            .iter_mut()
            .find(|(category, _)| category == &trade_tag.category_name)
        {
            Some((_, names)) => names.push(trade_tag.tag.name.clone()),
            None => grouped.push((
                trade_tag.category_name.clone(),
                vec![trade_tag.tag.name.clone()],
            )),
        }
    }
    for (category, names) in &grouped {
        push(category, &names.join(", "));
    }

    // Legacy freeform fields (frozen; populated only on old trades).
    push("entry_tactics", &entry.entry_tactics);
    push("edges_spotted", &entry.edges_spotted);
    push("mistakes", &entry.mistakes);
    if let Some(notes) = &entry.notes {
        push("notes", notes);
    }
    blocks
}

/// One `Field` block per non-empty playbook rule field, feeding the chunker.
fn playbook_blocks(book: &PlaybookWithStats) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut push = |name: &str, value: &str| {
        if !value.trim().is_empty() {
            blocks.push(Block::field(name, value));
        }
    };
    push("name", &book.name);
    push("edge", &book.edge_name);
    push("entry_rules", &book.entry_rules);
    push("exit_rules", &book.exit_rules);
    push("position_sizing_rules", &book.position_sizing_rules);
    if let Some(additional) = &book.additional_rules {
        push("additional_rules", additional);
    }
    blocks
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

async fn reindex_vectors_for_account(
    agents: &AgentsClient,
    vector_db: &VectorDatabaseClient,
    user_id: &str,
    account_id: &str,
    docs: &[(AiSourceDocument, Vec<Block>, DocMeta)],
) -> Result<()> {
    vector_db.ensure_hybrid_collection().await?;

    // Diff against what's already indexed so we only re-embed new/changed docs
    // (Voyage is rate-limited) and drop chunks of removed docs.
    let indexed = vector_db.indexed_source_hashes(user_id, account_id).await?;
    let current_ids: HashSet<&str> = docs.iter().map(|(d, _, _)| d.source_id.as_str()).collect();

    // Remove chunks (and their parents) for sources that no longer exist.
    for source_id in indexed.keys() {
        if !current_ids.contains(source_id.as_str()) {
            vector_db
                .delete_documents_by_source_id(user_id, account_id, source_id)
                .await?;
            vector_db
                .delete_parents_by_source_id(user_id, account_id, source_id)
                .await?;
        }
    }

    // Only (re)index docs that are new or whose content hash changed.
    let to_index: Vec<&(AiSourceDocument, Vec<Block>, DocMeta)> = docs
        .iter()
        .filter(|(d, _, _)| {
            indexed
                .get(&d.source_id)
                .is_none_or(|h| h != &d.content_hash)
        })
        .collect();

    if to_index.is_empty() {
        return Ok(());
    }

    // Clear any stale chunks (and parents) for each changed doc before re-indexing.
    for (doc, _, _) in &to_index {
        vector_db
            .delete_documents_by_source(user_id, account_id, &doc.source_type, &doc.source_id)
            .await?;
        vector_db
            .delete_parents_by_source(user_id, account_id, &doc.source_type, &doc.source_id)
            .await?;
    }

    let created_at = chrono::Utc::now().to_rfc3339();

    // Build (doc, raw chunk, enriched embed_text, parent link) tuples and parent
    // rows across all changed docs. Small child chunks stay the embedded unit;
    // each child links to a parent section returned at search time.
    struct Pending {
        doc_idx: usize,
        raw: String,
        embed_text: String,
        parent_id: String,
    }
    let mut pending: Vec<Pending> = Vec::new();
    let mut parents: Vec<VectorParentUpsert> = Vec::new();
    for (doc_idx, (doc, blocks, meta)) in to_index.iter().enumerate() {
        let chunks = chunk_blocks(blocks);
        if chunks.is_empty() {
            continue;
        }

        // Resolve the per-chunk LLM context blurbs for this doc: use the cache on a
        // complete hit, otherwise make ONE Gemini call (one per doc, never per
        // chunk) and persist it. `blurbs[chunk_index]` is the blurb (empty => none).
        // When the feature flag is off this stays empty and we embed deterministic
        // text only, exactly like Phase 2/3.
        let blurbs: Vec<String> = if ENABLE_LLM_CONTEXT {
            // A Gemini failure or malformed-JSON parse error for one doc degrades
            // that doc to deterministic-only context (the `ENABLE_LLM_CONTEXT=false`
            // path) instead of aborting the whole account reindex. The rest of the
            // docs and the batched embed + upsert proceed normally.
            match resolve_context_blurbs(agents, vector_db, doc, &chunks).await {
                Ok(blurbs) => blurbs,
                Err(error) => {
                    log::warn!(
                        "context blurb generation failed for source {source_id}, falling back to deterministic context: {error}",
                        source_id = doc.source_id,
                        error = error
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        let blurb_for = |idx: usize| -> Option<String> {
            blurbs.get(idx).filter(|b| !b.trim().is_empty()).cloned()
        };

        // Grouping: a short doc (single chunk) or one with no headings becomes a
        // single whole-doc parent; otherwise group by the top heading segment.
        let single_parent = chunks.len() == 1 || chunks.iter().all(|c| c.heading_path.is_empty());

        if single_parent {
            let parent_id = Uuid::new_v4().to_string();
            let content = chunks
                .iter()
                .map(|c| c.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            parents.push(VectorParentUpsert {
                id: parent_id.clone(),
                user_id: user_id.to_string(),
                account_id: account_id.to_string(),
                source_type: doc.source_type.clone(),
                source_id: doc.source_id.clone(),
                title: doc.title.clone(),
                content,
                created_at: created_at.clone(),
            });
            for chunk in &chunks {
                let header = deterministic_header(meta, &chunk.heading_path);
                let blurb = blurb_for(chunk.chunk_index);
                let embed_text = compose_embedded_text(&header, blurb.as_deref(), &chunk.text);
                pending.push(Pending {
                    doc_idx,
                    raw: chunk.text.clone(),
                    embed_text,
                    parent_id: parent_id.clone(),
                });
            }
        } else {
            // One parent per distinct top heading-path segment, in first-seen order.
            let mut section_ids: Vec<(String, String)> = Vec::new(); // (section_key, parent_id)
            let mut section_texts: HashMap<String, Vec<String>> = HashMap::new();
            for chunk in &chunks {
                let section_key = chunk.heading_path.first().cloned().unwrap_or_default();
                if !section_ids.iter().any(|(k, _)| k == &section_key) {
                    section_ids.push((section_key.clone(), Uuid::new_v4().to_string()));
                }
                section_texts
                    .entry(section_key.clone())
                    .or_default()
                    .push(chunk.text.clone());

                let parent_id = section_ids
                    .iter()
                    .find(|(k, _)| k == &section_key)
                    .map(|(_, id)| id.clone())
                    .expect("section id just inserted");
                let header = deterministic_header(meta, &chunk.heading_path);
                let blurb = blurb_for(chunk.chunk_index);
                let embed_text = compose_embedded_text(&header, blurb.as_deref(), &chunk.text);
                pending.push(Pending {
                    doc_idx,
                    raw: chunk.text.clone(),
                    embed_text,
                    parent_id,
                });
            }
            for (section_key, parent_id) in &section_ids {
                let content = section_texts
                    .get(section_key)
                    .map(|texts| texts.join("\n"))
                    .unwrap_or_default();
                parents.push(VectorParentUpsert {
                    id: parent_id.clone(),
                    user_id: user_id.to_string(),
                    account_id: account_id.to_string(),
                    source_type: doc.source_type.clone(),
                    source_id: doc.source_id.clone(),
                    title: doc.title.clone(),
                    content,
                    created_at: created_at.clone(),
                });
            }
        }
    }
    if pending.is_empty() {
        return Ok(());
    }

    // One batched Voyage call for all chunk embeddings (Voyage is rate-limited).
    let embeddings = vector_db
        .embed_texts(
            pending
                .iter()
                .map(|p| p.embed_text.clone())
                .collect::<Vec<_>>(),
            Some("document"),
        )
        .await?;

    let rows = pending
        .into_iter()
        .zip(embeddings)
        .map(|(p, dense)| {
            let (doc, _, _) = &to_index[p.doc_idx];
            VectorDocumentUpsert {
                id: Uuid::new_v4().to_string(),
                user_id: user_id.to_string(),
                account_id: account_id.to_string(),
                source_type: doc.source_type.clone(),
                source_id: doc.source_id.clone(),
                title: doc.title.clone(),
                content: p.raw,
                embed_text: p.embed_text,
                created_at: created_at.clone(),
                dense,
                source_content_hash: doc.content_hash.clone(),
                parent_id: Some(p.parent_id),
            }
        })
        .collect::<Vec<_>>();

    vector_db
        .upsert_parents(&parents)
        .await
        .context("failed to upsert ai source parents")?;

    vector_db
        .upsert_documents(&rows)
        .await
        .context("failed to upsert ai source vectors")?;

    Ok(())
}

/// Resolve the per-chunk LLM context blurbs for one source doc, returned indexed
/// by `chunk_index` (length == chunks.len(); empty string => no blurb). Uses the
/// `vector_context_cache` (keyed by `content_hash`) on a complete hit; otherwise
/// makes ONE Gemini call for the whole doc and persists the result.
async fn resolve_context_blurbs(
    agents: &AgentsClient,
    vector_db: &VectorDatabaseClient,
    doc: &AiSourceDocument,
    chunks: &[crate::service::ai::vector_database::chunking::Chunk],
) -> Result<Vec<String>> {
    let cached = vector_db.get_context_blurbs(&doc.content_hash).await?;
    let complete_hit = !chunks.is_empty()
        && chunks
            .iter()
            .all(|c| cached.contains_key(&(c.chunk_index as i32)));
    if complete_hit {
        return Ok(chunks
            .iter()
            .map(|c| {
                cached
                    .get(&(c.chunk_index as i32))
                    .cloned()
                    .unwrap_or_default()
            })
            .collect());
    }

    let chunk_texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    // Self-contained doc body for the prompt: the chunk texts joined.
    let doc_body = chunk_texts.join("\n\n");
    let blurbs = generate_context_blurbs(agents, &doc.title, &doc_body, &chunk_texts).await?;

    // Only persist real blurbs. `generate_context_blurbs` pads short model
    // responses to chunk count with empty strings; caching those would count as a
    // complete hit next time and permanently suppress regeneration. Filtering
    // empties means such chunks read as a cache miss and retry on a later reindex
    // (and meanwhile fall back to deterministic context via `blurb_for`).
    let pairs: Vec<(i32, String)> = chunks
        .iter()
        .zip(&blurbs)
        .filter(|(_, b)| !b.trim().is_empty())
        .map(|(c, b)| (c.chunk_index as i32, b.clone()))
        .collect();
    vector_db
        .put_context_blurbs(&doc.content_hash, &pairs)
        .await?;
    Ok(blurbs)
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
        log::debug!("retrieving vector results for query: {:?}", query);
        let vector = vector_db
            .embed_text(*query, Some("query"))
            .await
            .map_err(|e| {
                log::error!("embedding failed for query {:?}: {:?}", query, e);
                e
            })?;
        let hits = vector_db
            .dense_search_documents(&vector, user_id, account_id, per_query_limit)
            .await
            .map_err(|e| {
                log::error!(
                    "vector search failed for user_id={}, account_id={}, query={:?}: {:?}",
                    user_id,
                    account_id,
                    query,
                    e
                );
                e
            })
            .context("failed to query vector store for ai retrieval")?;

        for hit in hits {
            if hit.text.is_empty() {
                continue;
            }
            let key = format!("{}:{}:{}", hit.source_type, hit.source_id, hit.title);
            if seen.insert(key.clone()) {
                results.push((
                    format!("SRC-{}", seen.len()),
                    RetrievedChunk {
                        source_id: hit.source_id,
                        source_type: hit.source_type,
                        title: hit.title,
                        text: hit.text,
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
