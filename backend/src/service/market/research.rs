use anyhow::{Context, Result, anyhow};
use finance_query::{
    Capability, Frequency, Interval, Provider, Providers, StatementType, Ticker, TimeRange,
};
use serde::{Deserialize, Serialize};

const FMP_BASE_URL: &str = "https://financialmodelingprep.com";

#[derive(Debug, Clone, Serialize)]
pub struct SymbolSearchResult {
    pub symbol: String,
    pub name: String,
    pub exchange: Option<String>,
    pub security_type: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Candle {
    pub timestamp: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Article {
    pub title: String,
    pub url: String,
    pub source: String,
    pub published_at: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptRef {
    pub symbol: String,
    pub quarter: i32,
    pub year: i32,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transcript {
    pub symbol: String,
    pub quarter: i32,
    pub year: i32,
    pub date: Option<String>,
    pub content: String,
    pub source_url: String,
}

#[derive(Debug, Deserialize)]
struct FmpTranscriptRef {
    quarter: Option<i32>,
    #[serde(rename = "fiscalYear")]
    fiscal_year: Option<i32>,
    date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FmpTranscript {
    symbol: Option<String>,
    year: Option<i32>,
    date: Option<String>,
    content: Option<String>,
}

async fn configured_ticker(symbol: &str) -> finance_query::Result<Ticker> {
    if std::env::var("POLYGON_API_KEY").is_ok_and(|key| !key.trim().is_empty()) {
        let providers = Providers::builder()
            .route(Capability::QUOTE, [Provider::Polygon, Provider::Yahoo])
            .route(Capability::CHART, [Provider::Polygon, Provider::Yahoo])
            .route(Capability::CORPORATE, [Provider::Polygon, Provider::Yahoo])
            .route(
                Capability::FUNDAMENTALS,
                [Provider::Polygon, Provider::Yahoo],
            )
            .build()
            .await?;
        providers.ticker(symbol).build().await
    } else {
        Ticker::builder(symbol).build().await
    }
}

pub async fn search(query: &str) -> Result<Vec<SymbolSearchResult>> {
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }
    let options = finance_query::SearchOptions::new()
        .quotes_count(12)
        .enable_fuzzy_query(true)
        .enable_logo_url(true);
    let results = finance_query::finance::search(query, &options).await?;
    Ok(results
        .quotes
        .into_iter()
        .filter(|quote| {
            matches!(
                quote.quote_type.as_deref(),
                Some("EQUITY") | Some("ETF") | None
            )
        })
        .map(|quote| SymbolSearchResult {
            name: quote
                .long_name
                .or(quote.short_name)
                .unwrap_or_else(|| quote.symbol.clone()),
            symbol: quote.symbol,
            exchange: quote.exch_disp.or(quote.exchange),
            security_type: quote.type_disp.or(quote.quote_type),
        })
        .collect())
}

pub async fn chart(symbol: &str, range: &str) -> Result<Vec<Candle>> {
    let (interval, time_range) = match range {
        "1D" => (Interval::FiveMinutes, TimeRange::OneDay),
        "5D" => (Interval::ThirtyMinutes, TimeRange::FiveDays),
        "1M" => (Interval::OneDay, TimeRange::OneMonth),
        "6M" => (Interval::OneDay, TimeRange::SixMonths),
        "1Y" => (Interval::OneDay, TimeRange::OneYear),
        "5Y" => (Interval::OneWeek, TimeRange::FiveYears),
        _ => (Interval::OneDay, TimeRange::ThreeMonths),
    };
    let ticker = configured_ticker(symbol).await?;
    let chart = ticker.chart(interval, time_range).await?;
    Ok(chart
        .candles
        .into_iter()
        .map(|candle| Candle {
            timestamp: normalize_market_timestamp(candle.timestamp),
            open: candle.open,
            high: candle.high,
            low: candle.low,
            close: candle.close,
            volume: candle.volume,
        })
        .collect())
}

fn normalize_market_timestamp(timestamp: i64) -> i64 {
    // Polygon aggregate bars use milliseconds; Yahoo uses seconds.
    if timestamp.abs() >= 10_000_000_000 {
        timestamp / 1_000
    } else {
        timestamp
    }
}

pub async fn news(symbol: &str) -> Result<Vec<Article>> {
    let ticker = configured_ticker(symbol).await?;
    let articles = ticker.news().await?;
    Ok(articles
        .into_iter()
        .take(30)
        .map(|article| Article {
            title: article.title,
            url: article.link,
            source: article.source,
            published_at: article.time,
            image_url: (!article.img.is_empty()).then_some(article.img),
        })
        .collect())
}

pub async fn financials(symbol: &str) -> Result<serde_json::Value> {
    let ticker = configured_ticker(symbol).await?;
    let (income, balance, cash_flow) = tokio::join!(
        ticker.financials(StatementType::Income, Frequency::Annual),
        ticker.financials(StatementType::Balance, Frequency::Annual),
        ticker.financials(StatementType::CashFlow, Frequency::Annual),
    );
    Ok(serde_json::json!({
        "income": income.ok(),
        "balance": balance.ok(),
        "cashFlow": cash_flow.ok(),
    }))
}

pub async fn price(symbol: &str) -> Result<f64> {
    let ticker = configured_ticker(symbol).await?;
    let quote: finance_query::Quote = ticker.quote().await?;
    quote
        .regular_market_price
        .and_then(|value| value.raw)
        .ok_or_else(|| anyhow!("No market price returned for {symbol}"))
}

pub async fn company(symbol: &str) -> Result<serde_json::Value> {
    // Yahoo's quote-summary modules include the richer profile fields that are
    // not present in Polygon's price snapshot response.
    let ticker = Ticker::builder(symbol).logo().build().await?;
    let quote: finance_query::Quote = ticker.quote().await?;
    serde_json::to_value(quote).context("Unable to serialize company data")
}

fn fmp_key() -> Result<String> {
    std::env::var("FMP_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .ok_or_else(|| anyhow!("FMP_API_KEY is not configured"))
}

async fn fmp_get<T: for<'de> Deserialize<'de>>(path: &str, query: &[(&str, String)]) -> Result<T> {
    let key = fmp_key()?;
    let response = reqwest::Client::new()
        .get(format!("{FMP_BASE_URL}{path}"))
        .header("apikey", key)
        .query(query)
        .send()
        .await
        .context("FMP request failed")?;
    let status = response.status();
    let body = response
        .text()
        .await
        .context("Unable to read the FMP response")?;

    if !status.is_success() {
        let detail: String = body.chars().take(500).collect();
        return Err(anyhow!("FMP returned {status}: {detail}"));
    }

    serde_json::from_str(&body).context("Invalid FMP response")
}

pub async fn transcript_list(symbol: &str) -> Result<Vec<TranscriptRef>> {
    let rows: Vec<FmpTranscriptRef> = fmp_get(
        "/stable/earning-call-transcript-dates",
        &[("symbol", symbol.to_string())],
    )
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(TranscriptRef {
                symbol: symbol.to_string(),
                quarter: row.quarter?,
                year: row.fiscal_year?,
                date: row.date,
            })
        })
        .collect())
}

pub async fn transcript(symbol: &str, quarter: i32, year: i32) -> Result<Transcript> {
    if !(1..=4).contains(&quarter) {
        return Err(anyhow!("quarter must be between 1 and 4"));
    }
    let mut rows: Vec<FmpTranscript> = fmp_get(
        "/stable/earning-call-transcript",
        &[
            ("symbol", symbol.to_string()),
            ("quarter", quarter.to_string()),
            ("year", year.to_string()),
        ],
    )
    .await?;
    let row = rows
        .drain(..)
        .next()
        .ok_or_else(|| anyhow!("No transcript found for {symbol} Q{quarter} {year}"))?;
    Ok(Transcript {
        symbol: row.symbol.unwrap_or_else(|| symbol.to_string()),
        quarter,
        year: row.year.unwrap_or(year),
        date: row.date,
        content: row.content.unwrap_or_default(),
        source_url: format!(
            "https://financialmodelingprep.com/earnings-call-transcript/{symbol}/Q{quarter}/{year}"
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::normalize_market_timestamp;

    #[test]
    fn normalizes_provider_timestamps_to_seconds() {
        assert_eq!(normalize_market_timestamp(1_754_515_200), 1_754_515_200);
        assert_eq!(normalize_market_timestamp(1_754_515_200_000), 1_754_515_200);
    }
}
