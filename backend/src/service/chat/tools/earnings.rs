use anyhow::Result;
use finance_query::Ticker;
use serde::Deserialize;
use serde_json::json;

use crate::service::chat::types::{GroqFunctionDef, GroqToolDef};

#[derive(Debug, Deserialize)]
struct EarningsInput {
    symbol: String,
    include_transcript: Option<bool>,
    transcript_quarter: Option<String>,
    transcript_year: Option<i32>,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: GroqFunctionDef {
            name: "earnings".to_string(),
            description: "Get EPS history (actual vs estimated), upcoming earnings dates with estimates, and optionally an earnings call transcript for a stock.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "The stock ticker symbol (e.g. AAPL, MSFT)."
                    },
                    "include_transcript": {
                        "type": "boolean",
                        "description": "Whether to include the earnings call transcript. Defaults to false."
                    },
                    "transcript_quarter": {
                        "type": "string",
                        "description": "Fiscal quarter for the transcript (e.g. 'Q1', 'Q2', 'Q3', 'Q4'). If omitted, uses the latest."
                    },
                    "transcript_year": {
                        "type": "integer",
                        "description": "Fiscal year for the transcript (e.g. 2024). If omitted, uses the latest."
                    }
                },
                "required": ["symbol"]
            }),
        },
    }
}

pub async fn execute(arguments: &str) -> Result<String> {
    let input: EarningsInput = serde_json::from_str(arguments)?;
    let symbol = input.symbol.to_uppercase();

    let ticker = Ticker::new(&symbol).await?;
    let (earnings_opt, calendar_opt) =
        tokio::try_join!(ticker.earnings(), ticker.calendar_events())?;

    let mut output = format!("## Earnings — {}\n\n", symbol);

    // Quarterly EPS history
    if let Some(earnings) = &earnings_opt
        && let Some(chart) = &earnings.earnings_chart
        && !chart.quarterly.is_empty()
    {
        output.push_str("### Quarterly EPS History\n\n");
        output.push_str("| Quarter | Actual EPS | Estimated EPS | Surprise |\n");
        output.push_str("|---------|-----------|---------------|----------|\n");

        for q in &chart.quarterly {
            let date = q.date.as_deref().unwrap_or("N/A");
            let actual = q
                .actual
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");
            let estimate = q
                .estimate
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");
            let surprise = q.surprise_pct.as_deref().unwrap_or("N/A");
            output.push_str(&format!(
                "| {} | {} | {} | {} |\n",
                date, actual, estimate, surprise
            ));
        }
        output.push('\n');

        // Current quarter estimate
        if let Some(est) = &chart.current_quarter_estimate {
            let est_fmt = est.fmt.as_deref().unwrap_or("N/A");
            let est_date = chart
                .current_quarter_estimate_date
                .as_deref()
                .unwrap_or("N/A");
            let est_year = chart
                .current_quarter_estimate_year
                .map(|y| y.to_string())
                .unwrap_or_else(|| "N/A".to_string());
            output.push_str(&format!(
                "**Current Quarter Estimate ({est_date} {est_year}):** {est_fmt}\n\n"
            ));
        }
    }

    // Upcoming earnings from calendar events
    if let Some(calendar) = &calendar_opt {
        if let Some(cal_earnings) = &calendar.earnings {
            output.push_str("### Upcoming Earnings\n\n");

            if let Some(dates) = &cal_earnings.earnings_date
                && !dates.is_empty()
            {
                let date_strs: Vec<&str> = dates.iter().filter_map(|d| d.fmt.as_deref()).collect();
                output.push_str(&format!(
                    "**Earnings Date(s):** {}\n",
                    date_strs.join(" — ")
                ));
            }

            let avg_est = cal_earnings
                .earnings_average
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");
            let low_est = cal_earnings
                .earnings_low
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");
            let high_est = cal_earnings
                .earnings_high
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");
            let rev_avg = cal_earnings
                .revenue_average
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");

            output.push_str(&format!(
                "**EPS Estimate:** {avg_est} (range: {low_est} — {high_est})\n\
                **Revenue Estimate:** {rev_avg}\n\n"
            ));
        }

        // Optional: ex-dividend and dividend dates
        if let Some(ex_div) = &calendar.ex_dividend_date
            && let Some(fmt) = ex_div.fmt.as_deref()
        {
            output.push_str(&format!("**Ex-Dividend Date:** {}\n", fmt));
        }
        if let Some(div_date) = &calendar.dividend_date
            && let Some(fmt) = div_date.fmt.as_deref()
        {
            output.push_str(&format!("**Dividend Date:** {}\n", fmt));
        }
    }

    // Optional transcript
    if input.include_transcript.unwrap_or(false) {
        output.push_str("\n### Earnings Call Transcript\n\n");

        let quarter_ref = input.transcript_quarter.as_deref();
        let year_opt = input.transcript_year;

        match finance_query::finance::earnings_transcript(&symbol, quarter_ref, year_opt).await {
            Ok(transcript) => {
                let q = transcript.quarter();
                let yr = transcript.year();
                let text = transcript.text();
                let truncated = if text.len() > 3000 {
                    format!(
                        "{}...\n*(transcript truncated to 3000 chars)*",
                        &text[..3000]
                    )
                } else {
                    text.to_string()
                };
                output.push_str(&format!("**{q} {yr}**\n\n{truncated}\n"));
            }
            Err(e) => {
                output.push_str(&format!("*(Transcript unavailable: {e})*\n"));
            }
        }
    }

    Ok(output)
}
