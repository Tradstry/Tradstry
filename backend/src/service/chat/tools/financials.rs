use anyhow::Result;
use finance_query::Ticker;
use finance_query::{Frequency, StatementType};
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

use crate::service::chat::types::{LlmFunctionDef, LlmToolDef};

#[derive(Debug, Deserialize)]
struct FinancialsInput {
    symbol: String,
    statement_type: Option<String>,
    frequency: Option<String>,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "financials".to_string(),
            description: "Get financial statements (income, balance sheet, or cash flow) and key financial ratios (margins, ROE, debt/equity, etc.) for a stock.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "The stock ticker symbol (e.g. AAPL, MSFT)."
                    },
                    "statement_type": {
                        "type": "string",
                        "enum": ["income", "balance", "cashflow"],
                        "description": "Type of financial statement to retrieve. Defaults to 'income'."
                    },
                    "frequency": {
                        "type": "string",
                        "enum": ["annual", "quarterly"],
                        "description": "Data frequency. Defaults to 'annual'."
                    }
                },
                "required": ["symbol"]
            }),
        },
    }
}

pub async fn execute(arguments: &str) -> Result<String> {
    let input: FinancialsInput = serde_json::from_str(arguments)?;
    let symbol = input.symbol.to_uppercase();

    let statement_type = match input.statement_type.as_deref().unwrap_or("income") {
        "balance" => StatementType::Balance,
        "cashflow" => StatementType::CashFlow,
        _ => StatementType::Income,
    };

    let frequency = match input.frequency.as_deref().unwrap_or("annual") {
        "quarterly" => Frequency::Quarterly,
        _ => Frequency::Annual,
    };

    let ticker = Ticker::new(&symbol).await?;
    let (statement, fin_data_opt) = tokio::try_join!(
        ticker.financials(statement_type, frequency),
        ticker.financial_data()
    )?;

    let stmt_label = match statement_type {
        StatementType::Income => "Income Statement",
        StatementType::Balance => "Balance Sheet",
        StatementType::CashFlow => "Cash Flow Statement",
    };

    let freq_label = match frequency {
        Frequency::Annual => "Annual",
        Frequency::Quarterly => "Quarterly",
    };

    let mut output = format!(
        "## {} - {} {} ({})\n\n",
        symbol, freq_label, stmt_label, statement.frequency
    );

    // Collect and sort dates for column headers
    let mut all_dates: Vec<String> = statement
        .statement
        .values()
        .flat_map(|date_map| date_map.keys().cloned())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    all_dates.sort();
    all_dates.reverse();
    let display_dates: Vec<&str> = all_dates.iter().take(4).map(|s| s.as_str()).collect();

    // Sort metrics alphabetically using BTreeMap
    let sorted: BTreeMap<_, _> = statement.statement.iter().collect();

    output.push_str("| Metric |");
    for d in &display_dates {
        output.push_str(&format!(" {} |", d));
    }
    output.push('\n');

    output.push_str("|--------|");
    for _ in &display_dates {
        output.push_str("-----------|");
    }
    output.push('\n');

    for (metric, date_map) in &sorted {
        output.push_str(&format!("| {} |", metric));
        for date in &display_dates {
            let val = date_map.get(*date);
            let display = match val {
                Some(v) => format_large_number(*v),
                None => "N/A".to_string(),
            };
            output.push_str(&format!(" {} |", display));
        }
        output.push('\n');
    }

    // Key ratios section from FinancialData
    output.push_str("\n## Key Financial Ratios\n\n");

    let fd = fin_data_opt.as_ref();
    let gross_margins = fd
        .and_then(|d| d.gross_margins.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let operating_margins = fd
        .and_then(|d| d.operating_margins.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let profit_margins = fd
        .and_then(|d| d.profit_margins.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let ebitda_margins = fd
        .and_then(|d| d.ebitda_margins.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let roe = fd
        .and_then(|d| d.return_on_equity.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let roa = fd
        .and_then(|d| d.return_on_assets.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let debt_to_equity = fd
        .and_then(|d| d.debt_to_equity.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let current_ratio = fd
        .and_then(|d| d.current_ratio.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let revenue_growth = fd
        .and_then(|d| d.revenue_growth.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let earnings_growth = fd
        .and_then(|d| d.earnings_growth.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let total_cash = fd
        .and_then(|d| d.total_cash.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let total_debt = fd
        .and_then(|d| d.total_debt.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let free_cashflow = fd
        .and_then(|d| d.free_cashflow.as_ref())
        .and_then(|v| v.fmt.as_deref())
        .unwrap_or("N/A");
    let recommendation = fd
        .and_then(|d| d.recommendation_key.as_deref())
        .unwrap_or("N/A");

    output.push_str(&format!(
        "**Gross Margin:** {gross_margins} | **Operating Margin:** {operating_margins} | **Net Margin:** {profit_margins}\n\
        **EBITDA Margin:** {ebitda_margins}\n\
        **ROE:** {roe} | **ROA:** {roa}\n\
        **Debt/Equity:** {debt_to_equity} | **Current Ratio:** {current_ratio}\n\
        **Revenue Growth:** {revenue_growth} | **Earnings Growth:** {earnings_growth}\n\
        **Total Cash:** {total_cash} | **Total Debt:** {total_debt} | **Free Cash Flow:** {free_cashflow}\n\
        **Analyst Recommendation:** {recommendation}"
    ));

    Ok(output)
}

fn format_large_number(v: f64) -> String {
    let abs = v.abs();
    let sign = if v < 0.0 { "-" } else { "" };
    if abs >= 1_000_000_000_000.0 {
        format!("{}{:.2}T", sign, abs / 1_000_000_000_000.0)
    } else if abs >= 1_000_000_000.0 {
        format!("{}{:.2}B", sign, abs / 1_000_000_000.0)
    } else if abs >= 1_000_000.0 {
        format!("{}{:.2}M", sign, abs / 1_000_000.0)
    } else if abs >= 1_000.0 {
        format!("{}{:.2}K", sign, abs / 1_000.0)
    } else {
        format!("{}{:.2}", sign, abs)
    }
}
