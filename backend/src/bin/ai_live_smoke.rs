//! Live, read-only provider smoke checks for the AI stack.
//!
//! Exercises Gemini instruction accuracy, Voyage embeddings/reranking, the
//! Yahoo-backed AI quote tool, explicit Polygon quotes, FMP transcript metadata,
//! and a grounded Gemini synthesis. No database or provider mutations occur.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail, ensure};
use finance_query::{Capability, Provider, Providers};
use serde::Deserialize;
use tradstry_backend::service::ai::chat::tools::stock_quote;
use tradstry_backend::service::ai::client::AgentsClient;
use tradstry_backend::service::ai::vector_database::client::VectorDatabaseClient;
use tradstry_backend::service::market::research;

#[derive(Deserialize)]
struct GeminiExactCheck {
    arithmetic: i64,
    ordering: String,
    token: String,
}

#[derive(Deserialize)]
struct GroundedQuote {
    symbol: String,
    price: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let total_started = Instant::now();

    let agents = AgentsClient::from_env()?;
    let vector = VectorDatabaseClient::from_env_read_only()?;

    let gemini_started = Instant::now();
    let exact_raw = agents
        .prompt_with(
            "Return only valid minified JSON. Do every requested operation exactly; do not add markdown or commentary.",
            128,
            "Return an object with exactly these keys: arithmetic = 37 * 19 as an integer; ordering = the lexicographically earlier lowercase word among 'risk' and 'reward'; token = exactly LIVE_GEMINI_OK_73.",
        )
        .await
        .context("Gemini exactness check failed")?;
    let exact: GeminiExactCheck = parse_json_response(&exact_raw)
        .context("Gemini exactness check did not return the required JSON")?;
    ensure!(
        exact.arithmetic == 703,
        "Gemini arithmetic check was incorrect"
    );
    ensure!(
        exact.ordering == "reward",
        "Gemini ordering check was incorrect"
    );
    ensure!(
        exact.token == "LIVE_GEMINI_OK_73",
        "Gemini exact-token check was incorrect"
    );
    println!(
        "Gemini exact checks: 3/3 passed (model={}, latency_ms={:.0})",
        agents.model(),
        gemini_started.elapsed().as_secs_f64() * 1_000.0
    );

    let voyage_started = Instant::now();
    let embeddings = vector
        .embed_texts(
            [
                "Reduce position size when portfolio volatility rises.",
                "A sourdough loaf benefits from a long cold fermentation.",
            ],
            Some("document"),
        )
        .await
        .context("Voyage embeddings check failed")?;
    ensure!(
        embeddings.len() == 2,
        "Voyage returned the wrong embedding count"
    );
    let expected_dimension = vector.config().voyage.output_dimension as usize;
    ensure!(
        embeddings
            .iter()
            .all(|embedding| embedding.len() == expected_dimension),
        "Voyage returned an unexpected embedding dimension"
    );
    ensure!(
        embeddings.iter().flatten().all(|value| value.is_finite()),
        "Voyage returned a non-finite embedding value"
    );
    let reranked = vector
        .rerank(
            "How should I respond to rising portfolio volatility?",
            vec![
                "Reduce position size when portfolio volatility rises.".to_owned(),
                "A sourdough loaf benefits from a long cold fermentation.".to_owned(),
                "The museum closes at six in the evening.".to_owned(),
            ],
            Some(3),
        )
        .await
        .context("Voyage reranker check failed")?;
    ensure!(
        reranked.first().is_some_and(|result| result.index == 0),
        "Voyage reranker did not rank the relevant document first"
    );
    println!(
        "Voyage checks: embeddings valid, relevant rerank=1 (dimension={}, latency_ms={:.0})",
        expected_dimension,
        voyage_started.elapsed().as_secs_f64() * 1_000.0
    );

    let finance_started = Instant::now();
    let yahoo_future = stock_quote::execute(r#"{"symbol":"AAPL"}"#);
    let polygon_future = polygon_price("AAPL");
    let fmp_future = research::transcript_list("AAPL");
    let (yahoo_result, polygon_result, fmp_result) =
        tokio::join!(yahoo_future, polygon_future, fmp_future);

    let yahoo_quote = yahoo_result.context("Yahoo-backed AI stock quote check failed")?;
    let yahoo_price = extract_markdown_price(&yahoo_quote)?;
    ensure!(yahoo_price != "N/A", "Yahoo returned no current AAPL price");
    let polygon_price = polygon_result.context("Polygon quote check failed")?;
    ensure!(
        polygon_price.is_finite() && polygon_price > 0.0,
        "Polygon returned an invalid AAPL price"
    );
    let fmp_refs = fmp_result.context("FMP transcript-list check failed")?;
    ensure!(
        !fmp_refs.is_empty(),
        "FMP returned no AAPL transcript metadata"
    );
    println!(
        "Finance checks: Yahoo quote valid, Polygon quote valid, FMP transcript metadata={} (latency_ms={:.0})",
        fmp_refs.len(),
        finance_started.elapsed().as_secs_f64() * 1_000.0
    );

    let grounding_started = Instant::now();
    let grounded_raw = agents
        .prompt_with(
            "Answer using only the supplied tool result. Return only valid minified JSON with exactly the keys symbol and price. Copy both values exactly; do not calculate, round, add a currency sign, or add commentary.",
            128,
            &format!(
                "User question: What is AAPL's current price?\nTool result:\n{yahoo_quote}"
            ),
        )
        .await
        .context("grounded Gemini finance synthesis failed")?;
    let grounded: GroundedQuote = parse_json_response(&grounded_raw)
        .context("grounded finance synthesis returned invalid JSON")?;
    ensure!(
        grounded.symbol == "AAPL",
        "grounded answer changed the symbol"
    );
    ensure!(
        grounded.price == yahoo_price,
        "grounded answer changed the provider price"
    );
    println!(
        "Grounded answer check: exact tool-value preservation passed (latency_ms={:.0})",
        grounding_started.elapsed().as_secs_f64() * 1_000.0
    );
    println!(
        "All live provider checks passed (total_ms={:.0})",
        total_started.elapsed().as_secs_f64() * 1_000.0
    );
    Ok(())
}

async fn polygon_price(symbol: &str) -> Result<f64> {
    let providers = Providers::builder()
        .route(Capability::QUOTE, [Provider::Polygon])
        .timeout(Duration::from_secs(10))
        .build()
        .await
        .context("failed to initialize the Polygon provider")?;
    let ticker = providers
        .ticker(symbol)
        .timeout(Duration::from_secs(10))
        .build()
        .await
        .context("failed to build the Polygon ticker")?;
    let quote: finance_query::Quote = ticker
        .quote()
        .await
        .context("Polygon quote request failed")?;
    quote
        .regular_market_price
        .and_then(|value| value.raw)
        .ok_or_else(|| anyhow::anyhow!("Polygon returned no regular-market price"))
}

fn extract_markdown_price(quote: &str) -> Result<String> {
    let line = quote
        .lines()
        .find(|line| line.trim_start().starts_with("**Price:**"))
        .context("AI quote tool output did not contain a price line")?;
    let value = line
        .trim_start()
        .strip_prefix("**Price:**")
        .context("AI quote tool price format changed")?
        .split_whitespace()
        .next()
        .unwrap_or_default();
    if value.is_empty() {
        bail!("AI quote tool returned an empty price");
    }
    Ok(value.to_owned())
}

fn parse_json_response<T: for<'de> Deserialize<'de>>(raw: &str) -> Result<T> {
    let trimmed = raw.trim();
    if let Ok(value) = serde_json::from_str(trimmed) {
        return Ok(value);
    }
    let without_fence = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .and_then(|value| value.strip_suffix("```"))
        .map(str::trim)
        .context("response was neither JSON nor a single fenced JSON block")?;
    serde_json::from_str(without_fence).context("failed to parse fenced JSON")
}
