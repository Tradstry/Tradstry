use anyhow::Result;
use finance_query::Ticker;
use serde::Deserialize;
use serde_json::json;

use crate::service::chat::types::{GroqFunctionDef, GroqToolDef};

#[derive(Debug, Deserialize)]
struct StockNewsInput {
    symbol: Option<String>,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: GroqFunctionDef {
            name: "stock_news".to_string(),
            description: "Get the latest financial news for a specific stock symbol, or general market news if no symbol is provided.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "Optional stock ticker symbol (e.g. AAPL, TSLA). If omitted, returns general market news."
                    }
                },
                "required": []
            }),
        },
    }
}

pub async fn execute(arguments: &str) -> Result<String> {
    let input: StockNewsInput = serde_json::from_str(arguments)?;

    let articles = if let Some(symbol) = input.symbol.as_deref() {
        let symbol = symbol.to_uppercase();
        let ticker = Ticker::new(&symbol).await?;
        ticker.news().await?
    } else {
        finance_query::finance::news().await?
    };

    if articles.is_empty() {
        return Ok("No news articles found.".to_string());
    }

    let header = if let Some(sym) = &input.symbol {
        format!("## Latest News for {}\n\n", sym.to_uppercase())
    } else {
        "## General Market News\n\n".to_string()
    };

    let items: Vec<String> = articles
        .iter()
        .take(8)
        .map(|a| {
            format!(
                "**{}**\n*{}* — {}\n{}\n",
                a.title, a.source, a.time, a.link
            )
        })
        .collect();

    Ok(format!("{}{}", header, items.join("\n---\n\n")))
}
