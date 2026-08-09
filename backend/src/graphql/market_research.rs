use std::sync::Arc;

use async_graphql::{Context, Error, InputObject, Json, Object, Result, SimpleObject};
use chrono::Utc;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::service::ai::client::AgentsClient;
use crate::service::db::Db;
use crate::service::market::research;
use crate::service::notifications::{NotificationEvent, outbox};

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketSearchResultGql {
    symbol: String,
    name: String,
    exchange: Option<String>,
    security_type: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketCandleGql {
    timestamp: i64,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: i64,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketArticleGql {
    title: String,
    url: String,
    source: String,
    published_at: String,
    image_url: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketTranscriptRefGql {
    symbol: String,
    quarter: i32,
    year: i32,
    date: Option<String>,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketTranscriptGql {
    symbol: String,
    quarter: i32,
    year: i32,
    date: Option<String>,
    content: String,
    source_url: String,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketWatchlistGql {
    id: String,
    name: String,
    symbols: Vec<String>,
    created_at: String,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketReportGql {
    id: String,
    symbol: String,
    title: String,
    body: String,
    sources: Vec<String>,
    created_at: String,
}

#[derive(SimpleObject)]
#[graphql(rename_fields = "camelCase")]
pub struct MarketMonitorGql {
    id: String,
    symbol: String,
    name: String,
    condition: String,
    threshold: f64,
    enabled: bool,
    last_triggered_at: Option<String>,
    created_at: String,
}

#[derive(InputObject)]
#[graphql(rename_fields = "camelCase")]
pub struct CreateMarketMonitorInput {
    symbol: String,
    name: String,
    condition: String,
    threshold: f64,
}

async fn resolve_user(ctx: &Context<'_>) -> Result<(Arc<Db>, String)> {
    crate::graphql::auth::resolve_user(ctx).await
}

async fn require_workspace(db: &Db, user_id: &str, workspace_id: &str) -> Result<()> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM workspaces WHERE id = $1 AND user_id = $2)",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_one(db.pool())
    .await?;
    if !exists {
        return Err(Error::new("Workspace not found"));
    }
    Ok(())
}

fn normalize_symbol(value: &str) -> Result<String> {
    let value = value.trim().to_ascii_uppercase();
    if value.is_empty()
        || value.len() > 20
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
    {
        return Err(Error::new("Invalid market symbol"));
    }
    Ok(value)
}

fn string_list(value: Value) -> Vec<String> {
    serde_json::from_value(value).unwrap_or_default()
}

async fn watchlists(db: &Db, workspace_id: &str, user_id: &str) -> Result<Vec<MarketWatchlistGql>> {
    let rows = sqlx::query(
        "SELECT w.id, w.name, w.created_at, COALESCE(array_agg(s.symbol ORDER BY s.symbol) \
         FILTER (WHERE s.symbol IS NOT NULL), ARRAY[]::text[]) AS symbols \
         FROM market_watchlists w LEFT JOIN market_watchlist_symbols s ON s.watchlist_id = w.id \
         WHERE w.workspace_id = $1 AND w.user_id = $2 \
         GROUP BY w.id ORDER BY w.created_at",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(db.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(MarketWatchlistGql {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                symbols: row.try_get("symbols")?,
                created_at: row
                    .try_get::<chrono::DateTime<Utc>, _>("created_at")?
                    .to_rfc3339(),
            })
        })
        .collect()
}

async fn reports(db: &Db, workspace_id: &str, user_id: &str) -> Result<Vec<MarketReportGql>> {
    let rows = sqlx::query(
        "SELECT id, symbol, title, body, sources, created_at FROM market_reports \
         WHERE workspace_id = $1 AND user_id = $2 ORDER BY created_at DESC LIMIT 100",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(db.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(MarketReportGql {
                id: row.try_get("id")?,
                symbol: row.try_get("symbol")?,
                title: row.try_get("title")?,
                body: row.try_get("body")?,
                sources: string_list(row.try_get("sources")?),
                created_at: row
                    .try_get::<chrono::DateTime<Utc>, _>("created_at")?
                    .to_rfc3339(),
            })
        })
        .collect()
}

async fn monitors(db: &Db, workspace_id: &str, user_id: &str) -> Result<Vec<MarketMonitorGql>> {
    let rows = sqlx::query(
        "SELECT id, symbol, name, condition, threshold, enabled, last_triggered_at, created_at \
         FROM market_monitors WHERE workspace_id = $1 AND user_id = $2 ORDER BY created_at DESC",
    )
    .bind(workspace_id)
    .bind(user_id)
    .fetch_all(db.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            Ok(MarketMonitorGql {
                id: row.try_get("id")?,
                symbol: row.try_get("symbol")?,
                name: row.try_get("name")?,
                condition: row.try_get("condition")?,
                threshold: row.try_get("threshold")?,
                enabled: row.try_get("enabled")?,
                last_triggered_at: row
                    .try_get::<Option<chrono::DateTime<Utc>>, _>("last_triggered_at")?
                    .map(|value| value.to_rfc3339()),
                created_at: row
                    .try_get::<chrono::DateTime<Utc>, _>("created_at")?
                    .to_rfc3339(),
            })
        })
        .collect()
}

#[derive(Default)]
pub struct MarketResearchQuery;

#[Object]
impl MarketResearchQuery {
    async fn market_search(&self, query: String) -> Result<Vec<MarketSearchResultGql>> {
        Ok(research::search(&query)
            .await?
            .into_iter()
            .map(|item| MarketSearchResultGql {
                symbol: item.symbol,
                name: item.name,
                exchange: item.exchange,
                security_type: item.security_type,
            })
            .collect())
    }

    async fn market_chart(
        &self,
        symbol: String,
        range: Option<String>,
    ) -> Result<Vec<MarketCandleGql>> {
        let symbol = normalize_symbol(&symbol)?;
        Ok(research::chart(&symbol, range.as_deref().unwrap_or("3M"))
            .await?
            .into_iter()
            .map(|candle| MarketCandleGql {
                timestamp: candle.timestamp,
                open: candle.open,
                high: candle.high,
                low: candle.low,
                close: candle.close,
                volume: candle.volume,
            })
            .collect())
    }

    async fn market_news(&self, symbol: String) -> Result<Vec<MarketArticleGql>> {
        let symbol = normalize_symbol(&symbol)?;
        Ok(research::news(&symbol)
            .await?
            .into_iter()
            .map(|article| MarketArticleGql {
                title: article.title,
                url: article.url,
                source: article.source,
                published_at: article.published_at,
                image_url: article.image_url,
            })
            .collect())
    }

    async fn market_financials(&self, symbol: String) -> Result<Json<Value>> {
        Ok(Json(
            research::financials(&normalize_symbol(&symbol)?).await?,
        ))
    }

    async fn market_company(&self, symbol: String) -> Result<Json<Value>> {
        Ok(Json(research::company(&normalize_symbol(&symbol)?).await?))
    }

    async fn market_transcript_list(&self, symbol: String) -> Result<Vec<MarketTranscriptRefGql>> {
        let symbol = normalize_symbol(&symbol)?;
        Ok(research::transcript_list(&symbol)
            .await?
            .into_iter()
            .map(|item| MarketTranscriptRefGql {
                symbol: item.symbol,
                quarter: item.quarter,
                year: item.year,
                date: item.date,
            })
            .collect())
    }

    async fn market_transcript(
        &self,
        symbol: String,
        quarter: i32,
        year: i32,
    ) -> Result<MarketTranscriptGql> {
        let item = research::transcript(&normalize_symbol(&symbol)?, quarter, year).await?;
        Ok(MarketTranscriptGql {
            symbol: item.symbol,
            quarter: item.quarter,
            year: item.year,
            date: item.date,
            content: item.content,
            source_url: item.source_url,
        })
    }

    async fn market_watchlists(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<MarketWatchlistGql>> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        watchlists(&db, &workspace_id, &user_id).await
    }

    async fn market_reports(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<MarketReportGql>> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        reports(&db, &workspace_id, &user_id).await
    }

    async fn market_monitors(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<Vec<MarketMonitorGql>> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        monitors(&db, &workspace_id, &user_id).await
    }
}

#[derive(Default)]
pub struct MarketResearchMutation;

#[Object]
impl MarketResearchMutation {
    async fn create_market_watchlist(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        name: String,
    ) -> Result<MarketWatchlistGql> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        let name = name.trim();
        if name.is_empty() {
            return Err(Error::new("Watchlist name is required"));
        }
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO market_watchlists (id, workspace_id, user_id, name) VALUES ($1,$2,$3,$4)",
        )
        .bind(&id)
        .bind(&workspace_id)
        .bind(&user_id)
        .bind(name)
        .execute(db.pool())
        .await?;
        Ok(MarketWatchlistGql {
            id,
            name: name.to_string(),
            symbols: vec![],
            created_at: Utc::now().to_rfc3339(),
        })
    }

    async fn add_market_watchlist_symbol(
        &self,
        ctx: &Context<'_>,
        watchlist_id: String,
        symbol: String,
    ) -> Result<bool> {
        let (db, user_id) = resolve_user(ctx).await?;
        let result = sqlx::query(
            "INSERT INTO market_watchlist_symbols (watchlist_id, symbol) \
             SELECT id, $2 FROM market_watchlists WHERE id = $1 AND user_id = $3 \
             ON CONFLICT DO NOTHING",
        )
        .bind(watchlist_id)
        .bind(normalize_symbol(&symbol)?)
        .bind(user_id)
        .execute(db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn remove_market_watchlist_symbol(
        &self,
        ctx: &Context<'_>,
        watchlist_id: String,
        symbol: String,
    ) -> Result<bool> {
        let (db, user_id) = resolve_user(ctx).await?;
        let result = sqlx::query(
            "DELETE FROM market_watchlist_symbols s USING market_watchlists w \
             WHERE s.watchlist_id = w.id AND w.id = $1 AND w.user_id = $2 AND s.symbol = $3",
        )
        .bind(watchlist_id)
        .bind(user_id)
        .bind(normalize_symbol(&symbol)?)
        .execute(db.pool())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn create_market_monitor(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        input: CreateMarketMonitorInput,
    ) -> Result<MarketMonitorGql> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        let condition = input.condition.trim().to_ascii_uppercase();
        if !matches!(condition.as_str(), "ABOVE" | "BELOW") {
            return Err(Error::new("Condition must be ABOVE or BELOW"));
        }
        if !input.threshold.is_finite() || input.threshold <= 0.0 {
            return Err(Error::new("Threshold must be positive"));
        }
        let id = Uuid::new_v4().to_string();
        let name = input.name.trim().to_string();
        let symbol = normalize_symbol(&input.symbol)?;
        sqlx::query("INSERT INTO market_monitors (id, workspace_id, user_id, symbol, name, condition, threshold) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(&id).bind(&workspace_id).bind(&user_id).bind(&symbol).bind(&name).bind(&condition).bind(input.threshold).execute(db.pool()).await?;
        Ok(MarketMonitorGql {
            id,
            symbol,
            name,
            condition,
            threshold: input.threshold,
            enabled: true,
            last_triggered_at: None,
            created_at: Utc::now().to_rfc3339(),
        })
    }

    async fn delete_market_monitor(&self, ctx: &Context<'_>, id: String) -> Result<bool> {
        let (db, user_id) = resolve_user(ctx).await?;
        let result = sqlx::query("DELETE FROM market_monitors WHERE id = $1 AND user_id = $2")
            .bind(id)
            .bind(user_id)
            .execute(db.pool())
            .await?;
        Ok(result.rows_affected() > 0)
    }

    async fn generate_market_report(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
        symbol: String,
        focus: Option<String>,
    ) -> Result<MarketReportGql> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        let symbol = normalize_symbol(&symbol)?;
        let agents = ctx.data::<Arc<AgentsClient>>()?;
        let (news_result, financials_result, transcripts_result) = tokio::join!(
            research::news(&symbol),
            research::financials(&symbol),
            research::transcript_list(&symbol)
        );
        let news = news_result.unwrap_or_default();
        let financials = financials_result.unwrap_or_else(|_| serde_json::json!({}));
        let latest = transcripts_result.ok().and_then(|mut items| {
            items.sort_by_key(|item| (item.year, item.quarter));
            items.pop()
        });
        let transcript = if let Some(item) = latest {
            research::transcript(&symbol, item.quarter, item.year)
                .await
                .ok()
        } else {
            None
        };
        let mut sources: Vec<String> = news
            .iter()
            .take(8)
            .map(|article| article.url.clone())
            .collect();
        let financials_source =
            format!("https://api.polygon.io/vX/reference/financials?ticker={symbol}");
        sources.push(financials_source.clone());
        if let Some(item) = &transcript {
            sources.push(item.source_url.clone());
        }
        let mut seen = std::collections::HashSet::new();
        sources.retain(|source| seen.insert(source.clone()));
        let news_context = news
            .iter()
            .take(8)
            .enumerate()
            .map(|(index, article)| {
                format!(
                    "[{}] {} — {} ({})",
                    index + 1,
                    article.title,
                    article.source,
                    article.url
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        let transcript_context = transcript
            .as_ref()
            .map(|item| item.content.chars().take(18_000).collect::<String>())
            .unwrap_or_else(|| "Transcript unavailable".to_string());
        let financials_source_number = news.iter().take(8).count() + 1;
        let transcript_source_number = financials_source_number + 1;
        let prompt = format!(
            "Create a concise institutional-style research report for {symbol}. Focus: {}. Use only the supplied evidence. Every factual claim must cite a bracketed source number. Separate thesis, catalysts, risks, financial trend, earnings-call takeaways, and what would change the thesis. Never invent a citation.\n\nNEWS SOURCES:\n{news_context}\n\nFINANCIAL DATA [{financials_source_number}] {financials_source}:\n{}\n\nLATEST TRANSCRIPT [{transcript_source_number}] {}:\n{}",
            focus.as_deref().unwrap_or("general investment research"),
            serde_json::to_string(&financials)
                .unwrap_or_default()
                .chars()
                .take(14_000)
                .collect::<String>(),
            transcript
                .as_ref()
                .map(|item| item.source_url.as_str())
                .unwrap_or("unavailable"),
            transcript_context,
        );
        let body = agents.prompt_with("You are a rigorous equity research analyst. State uncertainty and distinguish facts from inference.", 12_000, &prompt).await?;
        let title = format!("{symbol} research brief");
        let id = Uuid::new_v4().to_string();
        sqlx::query("INSERT INTO market_reports (id, workspace_id, user_id, symbol, title, body, sources) VALUES ($1,$2,$3,$4,$5,$6,$7)")
            .bind(&id).bind(&workspace_id).bind(&user_id).bind(&symbol).bind(&title).bind(&body).bind(serde_json::json!(sources)).execute(db.pool()).await?;
        let event = NotificationEvent::ArtifactReady {
            workspace_id: workspace_id.clone(),
            kind: "market_report".to_string(),
            artifact_id: id.clone(),
        };
        outbox::record(db.pool(), &user_id, &event, Utc::now().date_naive()).await?;
        Ok(MarketReportGql {
            id,
            symbol,
            title,
            body,
            sources,
            created_at: Utc::now().to_rfc3339(),
        })
    }

    async fn evaluate_market_monitors(
        &self,
        ctx: &Context<'_>,
        workspace_id: String,
    ) -> Result<i32> {
        let (db, user_id) = resolve_user(ctx).await?;
        require_workspace(&db, &user_id, &workspace_id).await?;
        Ok(
            crate::service::market::monitor_worker::evaluate_workspace_once(
                &db,
                &user_id,
                &workspace_id,
            )
            .await? as i32,
        )
    }
}
