use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use async_graphql::{Error, Object, Result, SimpleObject, Subscription};
use chrono::{DateTime, Datelike, Timelike, Utc, Weekday};
use finance_query::{BatchQuotesResponse, Capability, Provider, Providers, Quote, Tickers};
use futures_util::StreamExt;
use tokio::sync::{Mutex, RwLock};

use crate::service::market::polygon;

const MAX_SYMBOLS: usize = 12;
const QUOTE_CACHE_TTL: Duration = Duration::from_secs(15);
const PROVIDER_TIMEOUT: Duration = Duration::from_secs(8);

#[derive(Clone, SimpleObject)]
pub struct MarketQuoteGql {
    symbol: String,
    name: String,
    price: Option<f64>,
    change: Option<f64>,
    change_percent: Option<f64>,
    regular_market_price: Option<f64>,
    pre_market_price: Option<f64>,
    post_market_price: Option<f64>,
    currency: Option<String>,
    currency_symbol: Option<String>,
    exchange: Option<String>,
    market_state: String,
    market_time: Option<String>,
    is_stale: bool,
}

#[derive(SimpleObject)]
pub struct MarketQuoteErrorGql {
    symbol: String,
    message: String,
}

#[derive(SimpleObject)]
pub struct MarketQuotesGql {
    quotes: Vec<MarketQuoteGql>,
    errors: Vec<MarketQuoteErrorGql>,
    fetched_at: String,
}

#[derive(SimpleObject)]
pub struct MarketPriceUpdateGql {
    symbol: String,
    price: f64,
    change: f64,
    change_percent: f64,
    currency: String,
    exchange: String,
    market_state: String,
    market_time: String,
}

#[derive(Clone)]
struct CachedQuote {
    quote: MarketQuoteGql,
    previous_close: Option<f64>,
    cached_at: Instant,
}

static QUOTE_CACHE: OnceLock<RwLock<HashMap<String, CachedQuote>>> = OnceLock::new();
static QUOTE_FETCH_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn quote_cache() -> &'static RwLock<HashMap<String, CachedQuote>> {
    QUOTE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn quote_fetch_lock() -> &'static Mutex<()> {
    QUOTE_FETCH_LOCK.get_or_init(|| Mutex::new(()))
}

fn normalize_symbols(symbols: Vec<String>) -> Result<Vec<String>> {
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();

    for symbol in symbols {
        let symbol = symbol.trim().to_ascii_uppercase();
        if symbol.is_empty() {
            continue;
        }
        if symbol.len() > 20
            || !symbol.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'^' | b'=')
            })
        {
            return Err(Error::new(format!("Invalid market symbol '{symbol}'")));
        }
        if seen.insert(symbol.clone()) {
            normalized.push(symbol);
        }
    }

    if normalized.is_empty() {
        return Err(Error::new("At least one market symbol is required"));
    }
    if normalized.len() > MAX_SYMBOLS {
        return Err(Error::new(format!(
            "A maximum of {MAX_SYMBOLS} market symbols can be requested"
        )));
    }
    Ok(normalized)
}

fn raw<T: Copy>(value: &Option<finance_query::FormattedValue<T>>) -> Option<T> {
    value.as_ref().and_then(|value| value.raw)
}

fn market_timestamp(timestamp: Option<i64>) -> Option<String> {
    timestamp
        .and_then(|timestamp| DateTime::<Utc>::from_timestamp(timestamp, 0))
        .map(|timestamp| timestamp.to_rfc3339())
}

fn market_quote(symbol: String, quote: Quote) -> MarketQuoteGql {
    let market_state = quote
        .market_state
        .clone()
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let (price, change, change_percent, market_time) = match market_state.as_str() {
        "PRE" | "PREPRE" => (
            raw(&quote.pre_market_price).or_else(|| raw(&quote.regular_market_price)),
            raw(&quote.pre_market_change).or_else(|| raw(&quote.regular_market_change)),
            raw(&quote.pre_market_change_percent)
                .or_else(|| raw(&quote.regular_market_change_percent)),
            market_timestamp(quote.pre_market_time.or(quote.regular_market_time)),
        ),
        "POST" | "POSTPOST" => (
            raw(&quote.post_market_price).or_else(|| raw(&quote.regular_market_price)),
            raw(&quote.post_market_change).or_else(|| raw(&quote.regular_market_change)),
            raw(&quote.post_market_change_percent)
                .or_else(|| raw(&quote.regular_market_change_percent)),
            market_timestamp(quote.post_market_time.or(quote.regular_market_time)),
        ),
        _ => (
            raw(&quote.regular_market_price),
            raw(&quote.regular_market_change),
            raw(&quote.regular_market_change_percent),
            market_timestamp(quote.regular_market_time),
        ),
    };

    MarketQuoteGql {
        name: quote
            .short_name
            .clone()
            .or(quote.long_name.clone())
            .unwrap_or_else(|| symbol.clone()),
        symbol,
        price,
        change,
        change_percent,
        regular_market_price: raw(&quote.regular_market_price),
        pre_market_price: raw(&quote.pre_market_price),
        post_market_price: raw(&quote.post_market_price),
        currency: quote.currency,
        currency_symbol: quote.currency_symbol,
        exchange: quote.exchange_name.or(quote.exchange),
        market_state,
        market_time,
        is_stale: false,
    }
}

fn quote_previous_close(quote: &Quote) -> Option<f64> {
    raw(&quote.regular_market_previous_close).or_else(|| raw(&quote.previous_close))
}

fn enrich_polygon_quote(quote: &mut Quote, fallback: Quote) {
    macro_rules! fill {
        ($field:ident) => {
            if quote.$field.is_none() {
                quote.$field = fallback.$field;
            }
        };
    }
    fill!(short_name);
    fill!(long_name);
    fill!(regular_market_change);
    fill!(regular_market_change_percent);
    fill!(regular_market_previous_close);
    fill!(previous_close);
    fill!(regular_market_time);
    fill!(pre_market_price);
    fill!(pre_market_change);
    fill!(pre_market_change_percent);
    fill!(pre_market_time);
    fill!(post_market_price);
    fill!(post_market_change);
    fill!(post_market_change_percent);
    fill!(post_market_time);
    fill!(currency);
    fill!(currency_symbol);
    fill!(exchange);
    fill!(exchange_name);
    fill!(market_state);
}

async fn fresh_cached_quotes(symbols: &[String]) -> (HashMap<String, MarketQuoteGql>, Vec<String>) {
    let cache = quote_cache().read().await;
    let mut quotes = HashMap::new();
    let mut missing = Vec::new();
    for symbol in symbols {
        match cache.get(symbol) {
            Some(cached) if cached.cached_at.elapsed() < QUOTE_CACHE_TTL => {
                quotes.insert(symbol.clone(), cached.quote.clone());
            }
            _ => missing.push(symbol.clone()),
        }
    }
    (quotes, missing)
}

async fn yahoo_quotes(symbols: Vec<String>) -> finance_query::Result<BatchQuotesResponse> {
    Tickers::builder(symbols)
        .timeout(PROVIDER_TIMEOUT)
        .build()
        .await?
        .quotes()
        .await
}

async fn provider_quotes(symbols: Vec<String>) -> finance_query::Result<BatchQuotesResponse> {
    let polygon_configured =
        std::env::var("POLYGON_API_KEY").is_ok_and(|key| !key.trim().is_empty());
    if !polygon_configured {
        return yahoo_quotes(symbols).await;
    }

    let polygon = Providers::builder()
        .route(Capability::QUOTE, [Provider::Polygon])
        .timeout(PROVIDER_TIMEOUT)
        .build()
        .await;
    let mut batch = match polygon {
        Ok(providers) => match providers
            .tickers(symbols.clone())
            .timeout(PROVIDER_TIMEOUT)
            .build()
            .await
        {
            Ok(tickers) => match tickers.quotes().await {
                Ok(batch) => batch,
                Err(error) => {
                    log::warn!(
                        "Polygon snapshot quote fetch failed, using Yahoo fallback: {error}"
                    );
                    return yahoo_quotes(symbols).await;
                }
            },
            Err(error) => {
                log::warn!("Polygon quote handle failed, using Yahoo fallback: {error}");
                return yahoo_quotes(symbols).await;
            }
        },
        Err(error) => {
            log::warn!("Polygon provider failed to initialize, using Yahoo fallback: {error}");
            return yahoo_quotes(symbols).await;
        }
    };

    match yahoo_quotes(symbols).await {
        Ok(fallback) => {
            for (symbol, fallback_quote) in fallback.quotes {
                batch.errors.remove(&symbol);
                match batch.quotes.get_mut(&symbol) {
                    Some(polygon_quote) => enrich_polygon_quote(polygon_quote, fallback_quote),
                    None => {
                        batch.quotes.insert(symbol, fallback_quote);
                    }
                }
            }
            for (symbol, error) in fallback.errors {
                if !batch.quotes.contains_key(&symbol) {
                    batch.errors.insert(symbol, error);
                }
            }
        }
        Err(error) => log::warn!("Yahoo snapshot quote enrichment failed: {error}"),
    }
    Ok(batch)
}

async fn get_market_quotes(symbols: Vec<String>) -> Result<MarketQuotesGql> {
    let symbols = normalize_symbols(symbols)?;
    let (mut quotes_by_symbol, mut missing) = fresh_cached_quotes(&symbols).await;
    let mut provider_errors = HashMap::new();

    if !missing.is_empty() {
        let _fetch_guard = quote_fetch_lock().lock().await;
        let (newly_cached, still_missing) = fresh_cached_quotes(&missing).await;
        quotes_by_symbol.extend(newly_cached);
        missing = still_missing;

        if !missing.is_empty() {
            match provider_quotes(missing.clone()).await {
                Ok(batch) => {
                    provider_errors.extend(batch.errors);
                    let now = Instant::now();
                    let mut cache = quote_cache().write().await;
                    for (symbol, quote) in batch.quotes {
                        let previous_close = quote_previous_close(&quote);
                        let quote = market_quote(symbol.clone(), quote);
                        cache.insert(
                            symbol.clone(),
                            CachedQuote {
                                quote: quote.clone(),
                                previous_close,
                                cached_at: now,
                            },
                        );
                        quotes_by_symbol.insert(symbol, quote);
                    }
                }
                Err(error) => {
                    log::warn!("market quote fetch failed: {error}");
                    for symbol in &missing {
                        provider_errors.insert(symbol.clone(), error.to_string());
                    }
                }
            }
        }
    }

    let cache = quote_cache().read().await;
    let mut quotes = Vec::new();
    let mut errors = Vec::new();
    for symbol in symbols {
        if let Some(quote) = quotes_by_symbol.remove(&symbol) {
            quotes.push(quote);
        } else if let Some(cached) = cache.get(&symbol) {
            let mut quote = cached.quote.clone();
            quote.is_stale = true;
            quotes.push(quote);
        } else {
            errors.push(MarketQuoteErrorGql {
                message: provider_errors.remove(&symbol).unwrap_or_else(|| {
                    "No quote was returned by the market-data provider".to_string()
                }),
                symbol,
            });
        }
    }

    Ok(MarketQuotesGql {
        quotes,
        errors,
        fetched_at: Utc::now().to_rfc3339(),
    })
}

#[derive(Clone)]
struct PriceBaseline {
    previous_close: Option<f64>,
    currency: String,
    exchange: String,
}

async fn price_baselines(symbols: &[String]) -> HashMap<String, PriceBaseline> {
    let cache = quote_cache().read().await;
    symbols
        .iter()
        .map(|symbol| {
            let cached = cache.get(symbol);
            let quote = cached.map(|cached| &cached.quote);
            (
                symbol.clone(),
                PriceBaseline {
                    previous_close: cached.and_then(|cached| cached.previous_close),
                    currency: quote
                        .and_then(|quote| quote.currency.clone())
                        .unwrap_or_else(|| "USD".to_string()),
                    exchange: quote
                        .and_then(|quote| quote.exchange.clone())
                        .unwrap_or_else(|| "Polygon".to_string()),
                },
            )
        })
        .collect()
}

fn market_state_at(timestamp_ms: i64) -> &'static str {
    let timestamp = DateTime::<Utc>::from_timestamp_millis(timestamp_ms).unwrap_or_else(Utc::now);
    let eastern = timestamp.with_timezone(&chrono_tz::America::New_York);
    if matches!(eastern.weekday(), Weekday::Sat | Weekday::Sun) {
        return "CLOSED";
    }
    let minute = eastern.hour() * 60 + eastern.minute();
    match minute {
        240..570 => "PRE",
        570..960 => "REGULAR",
        960..1200 => "POST",
        _ => "CLOSED",
    }
}

fn polygon_market_update(
    update: polygon::PriceUpdate,
    baselines: &HashMap<String, PriceBaseline>,
) -> MarketPriceUpdateGql {
    let baseline = baselines.get(&update.symbol);
    let previous_close = baseline.and_then(|baseline| baseline.previous_close);
    let change = previous_close.map_or(0.0, |close| update.price - close);
    let change_percent = previous_close
        .filter(|close| *close != 0.0)
        .map_or(0.0, |close| change / close * 100.0);
    MarketPriceUpdateGql {
        symbol: update.symbol,
        price: update.price,
        change,
        change_percent,
        currency: baseline
            .map(|baseline| baseline.currency.clone())
            .unwrap_or_else(|| "USD".to_string()),
        exchange: baseline
            .map(|baseline| baseline.exchange.clone())
            .unwrap_or_else(|| "Polygon".to_string()),
        market_state: market_state_at(update.timestamp_ms).to_string(),
        market_time: DateTime::<Utc>::from_timestamp_millis(update.timestamp_ms)
            .unwrap_or_else(Utc::now)
            .to_rfc3339(),
    }
}

#[derive(Default)]
pub struct MarketQuery;

#[Object]
impl MarketQuery {
    async fn market_quotes(&self, symbols: Vec<String>) -> Result<MarketQuotesGql> {
        get_market_quotes(symbols).await
    }
}

#[derive(Default)]
pub struct MarketSubscription;

#[Subscription]
impl MarketSubscription {
    async fn market_price_updates(
        &self,
        symbols: Vec<String>,
    ) -> Result<impl futures_util::Stream<Item = MarketPriceUpdateGql>> {
        let symbols = normalize_symbols(symbols)?;
        let baselines = price_baselines(&symbols).await;
        let stream = polygon::subscribe(symbols).await.map_err(|error| {
            Error::new(format!("Unable to start Polygon market stream: {error}"))
        })?;
        Ok(stream.map(move |update| polygon_market_update(update, &baselines)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbols_are_normalized_and_deduplicated() {
        assert_eq!(
            normalize_symbols(vec![" aapl ".into(), "AAPL".into(), "brk-b".into()]).unwrap(),
            vec!["AAPL", "BRK-B"]
        );
    }

    #[test]
    fn invalid_and_oversized_symbol_lists_are_rejected() {
        assert!(normalize_symbols(vec!["AAPL/USD".into()]).is_err());
        assert!(
            normalize_symbols((0..=MAX_SYMBOLS).map(|index| format!("S{index}")).collect())
                .is_err()
        );
    }

    #[test]
    fn polygon_updates_use_the_snapshot_previous_close() {
        let baselines = HashMap::from([(
            "AAPL".to_string(),
            PriceBaseline {
                previous_close: Some(180.0),
                currency: "USD".to_string(),
                exchange: "NASDAQ".to_string(),
            },
        )]);
        let update = polygon_market_update(
            polygon::PriceUpdate {
                symbol: "AAPL".to_string(),
                price: 189.0,
                timestamp_ms: 1_705_363_200_000,
            },
            &baselines,
        );
        assert_eq!(update.change, 9.0);
        assert_eq!(update.change_percent, 5.0);
    }
}
