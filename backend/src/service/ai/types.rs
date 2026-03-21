use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

pub const JOB_REINDEX_ACCOUNT_SOURCES: &str = "reindex_account_sources";
pub const JOB_GENERATE_AI_INSIGHTS: &str = "generate_ai_insights";
pub const JOB_GENERATE_AI_REPORT: &str = "generate_ai_report";
pub const JOB_GENERATE_MINDSET_SUMMARY: &str = "generate_mindset_summary";

pub const ARTIFACT_AI_INSIGHTS: &str = "ai_insights";
pub const ARTIFACT_AI_REPORT: &str = "ai_report";
pub const ARTIFACT_MINDSET_SUMMARY: &str = "mindset_summary";

pub type AiEventBus = broadcast::Sender<AiEventEnvelope>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiRange {
    Last7Days,
    Last30Days,
    YearToDate,
    Last1Year,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiTimeFilter {
    pub range: AiRange,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

impl Default for AiTimeFilter {
    fn default() -> Self {
        Self {
            range: AiRange::Last30Days,
            start_date: None,
            end_date: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceCitation {
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightCard {
    pub title: String,
    pub summary: String,
    pub category: String,
    pub severity: String,
    pub citations: Vec<SourceCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InsightBundle {
    pub overview: String,
    pub cards: Vec<InsightCard>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportSection {
    pub heading: String,
    pub body: String,
    pub citations: Vec<SourceCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportArtifact {
    pub title: String,
    pub summary: String,
    pub sections: Vec<ReportSection>,
    pub next_actions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MindsetSignal {
    pub pattern: String,
    pub evidence: String,
    pub coaching: String,
    pub citations: Vec<SourceCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MindsetSummary {
    pub overview: String,
    pub signals: Vec<MindsetSignal>,
    pub routines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiArtifactEnvelope {
    pub artifact_type: String,
    pub insight_bundle: Option<InsightBundle>,
    pub report: Option<ReportArtifact>,
    pub mindset_summary: Option<MindsetSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiJobHandle {
    pub job_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiJobRecord {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub job_type: String,
    pub artifact_type: Option<String>,
    pub time_filter_json: String,
    pub payload_json: String,
    pub status: String,
    pub error_message: Option<String>,
    pub created_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSourceDocument {
    pub id: String,
    pub user_id: String,
    pub account_id: String,
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub body_text: String,
    pub metadata_json: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiEventEnvelope {
    pub user_id: String,
    pub job_id: String,
    pub account_id: String,
    pub artifact_type: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub artifact: Option<AiArtifactEnvelope>,
    pub error: Option<String>,
}
