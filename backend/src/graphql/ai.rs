use std::sync::Arc;

use async_graphql::{Context, InputObject, Object, Result, SimpleObject, Subscription};
use clerk_rs::validators::authorizer::ClerkJwt;
use futures_util::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::service::{
    ai::{
        db as ai_db, jobs as ai_jobs,
        types::{
            ARTIFACT_AI_INSIGHTS, ARTIFACT_AI_REPORT, ARTIFACT_MINDSET_SUMMARY, AiArtifactEnvelope,
            AiEventBus, AiEventEnvelope, AiJobHandle, AiRange, AiTimeFilter, InsightBundle,
            InsightCard, JOB_GENERATE_AI_INSIGHTS, JOB_GENERATE_AI_REPORT,
            JOB_GENERATE_MINDSET_SUMMARY, MindsetSignal, MindsetSummary, ReportArtifact,
            ReportSection, SourceCitation,
        },
    },
    read_service::users::ensure_user,
    turso::TursoClient,
};

async fn resolve_user(ctx: &Context<'_>) -> Result<(Arc<TursoClient>, String)> {
    let jwt = ctx.data::<ClerkJwt>()?;
    let turso = ctx.data::<Arc<TursoClient>>()?;
    let conn = turso.get_connection()?;

    let full_name = jwt
        .other
        .get("full_name")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    let email = jwt
        .other
        .get("email")
        .and_then(|value| value.as_str())
        .unwrap_or("");

    let user = ensure_user(&conn, &jwt.sub, full_name, email).await?;
    Ok((turso.clone(), user.id))
}

#[derive(InputObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct AiTimeFilterInput {
    pub range: AiRangeInput,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(async_graphql::Enum, Copy, Clone, Eq, PartialEq)]
#[graphql(rename_items = "SCREAMING_SNAKE_CASE")]
pub enum AiRangeInput {
    Last7Days,
    Last30Days,
    YearToDate,
    Last1Year,
    Custom,
}

impl From<AiTimeFilterInput> for AiTimeFilter {
    fn from(value: AiTimeFilterInput) -> Self {
        let range = match value.range {
            AiRangeInput::Last7Days => AiRange::Last7Days,
            AiRangeInput::Last30Days => AiRange::Last30Days,
            AiRangeInput::YearToDate => AiRange::YearToDate,
            AiRangeInput::Last1Year => AiRange::Last1Year,
            AiRangeInput::Custom => AiRange::Custom,
        };
        Self {
            range,
            start_date: value.start_date,
            end_date: value.end_date,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct SourceCitationGql {
    pub source_type: String,
    pub source_id: String,
    pub title: String,
    pub excerpt: String,
}

impl From<SourceCitation> for SourceCitationGql {
    fn from(value: SourceCitation) -> Self {
        Self {
            source_type: value.source_type,
            source_id: value.source_id,
            title: value.title,
            excerpt: value.excerpt,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct InsightCardGql {
    pub title: String,
    pub summary: String,
    pub category: String,
    pub severity: String,
    pub citations: Vec<SourceCitationGql>,
}

impl From<InsightCard> for InsightCardGql {
    fn from(value: InsightCard) -> Self {
        Self {
            title: value.title,
            summary: value.summary,
            category: value.category,
            severity: value.severity,
            citations: value.citations.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct InsightBundleGql {
    pub overview: String,
    pub cards: Vec<InsightCardGql>,
    pub next_actions: Vec<String>,
}

impl From<InsightBundle> for InsightBundleGql {
    fn from(value: InsightBundle) -> Self {
        Self {
            overview: value.overview,
            cards: value.cards.into_iter().map(Into::into).collect(),
            next_actions: value.next_actions,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct ReportSectionGql {
    pub heading: String,
    pub body: String,
    pub citations: Vec<SourceCitationGql>,
}

impl From<ReportSection> for ReportSectionGql {
    fn from(value: ReportSection) -> Self {
        Self {
            heading: value.heading,
            body: value.body,
            citations: value.citations.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct ReportArtifactGql {
    pub title: String,
    pub summary: String,
    pub sections: Vec<ReportSectionGql>,
    pub next_actions: Vec<String>,
}

impl From<ReportArtifact> for ReportArtifactGql {
    fn from(value: ReportArtifact) -> Self {
        Self {
            title: value.title,
            summary: value.summary,
            sections: value.sections.into_iter().map(Into::into).collect(),
            next_actions: value.next_actions,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct MindsetSignalGql {
    pub pattern: String,
    pub evidence: String,
    pub coaching: String,
    pub citations: Vec<SourceCitationGql>,
}

impl From<MindsetSignal> for MindsetSignalGql {
    fn from(value: MindsetSignal) -> Self {
        Self {
            pattern: value.pattern,
            evidence: value.evidence,
            coaching: value.coaching,
            citations: value.citations.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct MindsetSummaryGql {
    pub overview: String,
    pub signals: Vec<MindsetSignalGql>,
    pub routines: Vec<String>,
}

impl From<MindsetSummary> for MindsetSummaryGql {
    fn from(value: MindsetSummary) -> Self {
        Self {
            overview: value.overview,
            signals: value.signals.into_iter().map(Into::into).collect(),
            routines: value.routines,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct AiArtifactEnvelopeGql {
    pub artifact_type: String,
    pub insight_bundle: Option<InsightBundleGql>,
    pub report: Option<ReportArtifactGql>,
    pub mindset_summary: Option<MindsetSummaryGql>,
}

impl From<AiArtifactEnvelope> for AiArtifactEnvelopeGql {
    fn from(value: AiArtifactEnvelope) -> Self {
        Self {
            artifact_type: value.artifact_type,
            insight_bundle: value.insight_bundle.map(Into::into),
            report: value.report.map(Into::into),
            mindset_summary: value.mindset_summary.map(Into::into),
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct AiJobHandleGql {
    pub job_id: String,
    pub status: String,
}

impl From<AiJobHandle> for AiJobHandleGql {
    fn from(value: AiJobHandle) -> Self {
        Self {
            job_id: value.job_id,
            status: value.status,
        }
    }
}

#[derive(SimpleObject, Clone)]
#[graphql(rename_fields = "camelCase")]
pub struct AiJobEventGql {
    pub job_id: String,
    pub account_id: String,
    pub artifact_type: Option<String>,
    pub status: String,
    pub message: Option<String>,
    pub artifact: Option<AiArtifactEnvelopeGql>,
    pub error: Option<String>,
}

impl From<AiEventEnvelope> for AiJobEventGql {
    fn from(value: AiEventEnvelope) -> Self {
        Self {
            job_id: value.job_id,
            account_id: value.account_id,
            artifact_type: value.artifact_type,
            status: value.status,
            message: value.message,
            artifact: value.artifact.map(Into::into),
            error: value.error,
        }
    }
}

#[derive(Default)]
pub struct AiQuery;

#[Object]
impl AiQuery {
    async fn ai_insights(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        time_filter: AiTimeFilterInput,
    ) -> Result<Option<AiArtifactEnvelopeGql>> {
        let (turso, user_id) = resolve_user(ctx).await?;
        ai_db::ensure_account_exists_for_user(&turso, &user_id, &account_id).await?;
        Ok(ai_db::get_latest_artifact(
            &turso,
            &user_id,
            &account_id,
            ARTIFACT_AI_INSIGHTS,
            &time_filter.into(),
        )
        .await?
        .map(Into::into))
    }

    async fn ai_report(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        time_filter: AiTimeFilterInput,
    ) -> Result<Option<AiArtifactEnvelopeGql>> {
        let (turso, user_id) = resolve_user(ctx).await?;
        ai_db::ensure_account_exists_for_user(&turso, &user_id, &account_id).await?;
        Ok(ai_db::get_latest_artifact(
            &turso,
            &user_id,
            &account_id,
            ARTIFACT_AI_REPORT,
            &time_filter.into(),
        )
        .await?
        .map(Into::into))
    }

    async fn mindset_summary(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        time_filter: AiTimeFilterInput,
    ) -> Result<Option<AiArtifactEnvelopeGql>> {
        let (turso, user_id) = resolve_user(ctx).await?;
        ai_db::ensure_account_exists_for_user(&turso, &user_id, &account_id).await?;
        Ok(ai_db::get_latest_artifact(
            &turso,
            &user_id,
            &account_id,
            ARTIFACT_MINDSET_SUMMARY,
            &time_filter.into(),
        )
        .await?
        .map(Into::into))
    }

    async fn ai_job(&self, ctx: &Context<'_>, job_id: String) -> Result<Option<AiJobHandleGql>> {
        let (turso, user_id) = resolve_user(ctx).await?;
        Ok(ai_db::get_job_for_user(&turso, &user_id, &job_id)
            .await?
            .map(|job| AiJobHandleGql {
                job_id: job.id,
                status: job.status,
            }))
    }
}

#[derive(Default)]
pub struct AiMutation;

#[Object]
impl AiMutation {
    async fn refresh_ai_insights(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        time_filter: AiTimeFilterInput,
    ) -> Result<AiJobHandleGql> {
        let (turso, user_id) = resolve_user(ctx).await?;
        ai_db::ensure_account_exists_for_user(&turso, &user_id, &account_id).await?;
        let time_filter: AiTimeFilter = time_filter.into();
        ai_jobs::enqueue_account_reindex(turso.as_ref(), &user_id, &account_id).await?;
        let handle = ai_db::enqueue_job(
            &turso,
            &user_id,
            &account_id,
            JOB_GENERATE_AI_INSIGHTS,
            Some(ARTIFACT_AI_INSIGHTS),
            &time_filter,
            &serde_json::json!({}),
            Some(&format!(
                "ai-insights:{user_id}:{account_id}:{}",
                serde_json::to_string(&time_filter)?
            )),
        )
        .await?;
        Ok(handle.into())
    }

    async fn generate_ai_report(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        time_filter: AiTimeFilterInput,
    ) -> Result<AiJobHandleGql> {
        let (turso, user_id) = resolve_user(ctx).await?;
        ai_db::ensure_account_exists_for_user(&turso, &user_id, &account_id).await?;
        let time_filter: AiTimeFilter = time_filter.into();
        ai_jobs::enqueue_account_reindex(turso.as_ref(), &user_id, &account_id).await?;
        let handle = ai_db::enqueue_job(
            &turso,
            &user_id,
            &account_id,
            JOB_GENERATE_AI_REPORT,
            Some(ARTIFACT_AI_REPORT),
            &time_filter,
            &serde_json::json!({}),
            Some(&format!(
                "ai-report:{user_id}:{account_id}:{}",
                serde_json::to_string(&time_filter)?
            )),
        )
        .await?;
        Ok(handle.into())
    }

    async fn refresh_mindset_summary(
        &self,
        ctx: &Context<'_>,
        account_id: String,
        time_filter: AiTimeFilterInput,
    ) -> Result<AiJobHandleGql> {
        let (turso, user_id) = resolve_user(ctx).await?;
        ai_db::ensure_account_exists_for_user(&turso, &user_id, &account_id).await?;
        let time_filter: AiTimeFilter = time_filter.into();
        ai_jobs::enqueue_account_reindex(turso.as_ref(), &user_id, &account_id).await?;
        let handle = ai_db::enqueue_job(
            &turso,
            &user_id,
            &account_id,
            JOB_GENERATE_MINDSET_SUMMARY,
            Some(ARTIFACT_MINDSET_SUMMARY),
            &time_filter,
            &serde_json::json!({}),
            Some(&format!(
                "mindset:{user_id}:{account_id}:{}",
                serde_json::to_string(&time_filter)?
            )),
        )
        .await?;
        Ok(handle.into())
    }
}

#[derive(Default)]
pub struct AiSubscription;

#[Subscription]
impl AiSubscription {
    async fn ai_job_events(
        &self,
        ctx: &Context<'_>,
        job_id: String,
    ) -> Result<impl futures_util::Stream<Item = AiJobEventGql>> {
        let (_, user_id) = resolve_user(ctx).await?;
        let event_bus = ctx.data::<AiEventBus>()?.clone();
        Ok(
            BroadcastStream::new(event_bus.subscribe()).filter_map(move |item| {
                let user_id = user_id.clone();
                let job_id = job_id.clone();
                async move {
                    match item.ok() {
                        Some(event) if event.user_id == user_id && event.job_id == job_id => {
                            Some(event.into())
                        }
                        _ => None,
                    }
                }
            }),
        )
    }
}
