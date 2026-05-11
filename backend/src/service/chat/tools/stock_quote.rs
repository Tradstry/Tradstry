use anyhow::Result;
use finance_query::Ticker;
use serde::Deserialize;
use serde_json::json;

use crate::service::chat::types::{LlmFunctionDef, LlmToolDef};

#[derive(Debug, Deserialize)]
struct StockQuoteInput {
    symbol: String,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "stock_quote".to_string(),
            description: "Get real-time stock price, market cap, volume, PE ratio, 52-week range, dividend yield, and beta for a given ticker symbol.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "The stock ticker symbol (e.g. AAPL, MSFT, TSLA)."
                    }
                },
                "required": ["symbol"]
            }),
        },
    }
}

pub async fn execute(arguments: &str) -> Result<String> {
    let input: StockQuoteInput = serde_json::from_str(arguments)?;
    let symbol = input.symbol.to_uppercase();

    let ticker = Ticker::new(&symbol).await?;
    let (price_opt, detail_opt) = tokio::try_join!(ticker.price(), ticker.summary_detail())?;

    let price = price_opt.unwrap_or_default();

    let name = price
        .long_name
        .as_deref()
        .or(price.short_name.as_deref())
        .unwrap_or(&symbol);

    let current = price
        .regular_market_price
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let change = price
        .regular_market_change
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let change_pct = price
        .regular_market_change_percent
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let volume = price
        .regular_market_volume
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let market_cap = price
        .market_cap
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let open = price
        .regular_market_open
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let prev_close = price
        .regular_market_previous_close
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let day_high = price
        .regular_market_day_high
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let day_low = price
        .regular_market_day_low
        .as_ref()
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let market_state = price.market_state.as_deref().unwrap_or("UNKNOWN");
    let exchange = price.exchange_name.as_deref().unwrap_or("N/A");
    let currency = price.currency.as_deref().unwrap_or("USD");

    let week52_high = detail_opt
        .as_ref()
        .and_then(|d| d.fifty_two_week_high.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let week52_low = detail_opt
        .as_ref()
        .and_then(|d| d.fifty_two_week_low.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let trailing_pe = detail_opt
        .as_ref()
        .and_then(|d| d.trailing_pe.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let forward_pe = detail_opt
        .as_ref()
        .and_then(|d| d.forward_pe.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let beta = detail_opt
        .as_ref()
        .and_then(|d| d.beta.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let dividend_yield = detail_opt
        .as_ref()
        .and_then(|d| d.trailing_annual_dividend_yield.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    let avg_volume = detail_opt
        .as_ref()
        .and_then(|d| d.average_volume.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");

    Ok(format!(
        "## {name} ({symbol})\n\
        **Exchange:** {exchange} | **Currency:** {currency} | **Market State:** {market_state}\n\n\
        **Price:** {current} ({change} / {change_pct})\n\
        **Open:** {open} | **Prev Close:** {prev_close}\n\
        **Day Range:** {day_low} - {day_high}\n\
        **52-Week Range:** {week52_low} - {week52_high}\n\n\
        **Volume:** {volume} | **Avg Volume:** {avg_volume}\n\
        **Market Cap:** {market_cap}\n\n\
        **Trailing P/E:** {trailing_pe} | **Forward P/E:** {forward_pe}\n\
        **Beta:** {beta} | **Dividend Yield:** {dividend_yield}"
    ))
}
