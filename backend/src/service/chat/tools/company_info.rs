use anyhow::Result;
use finance_query::Ticker;
use serde::Deserialize;
use serde_json::json;

use crate::service::chat::types::{GroqFunctionDef, GroqToolDef};

#[derive(Debug, Deserialize)]
struct CompanyInfoInput {
    symbol: String,
}

pub fn schema() -> GroqToolDef {
    GroqToolDef {
        tool_type: "function".to_string(),
        function: GroqFunctionDef {
            name: "company_info".to_string(),
            description: "Get company profile including sector, industry, location, employee count, website, key executives, governance risk scores, and business summary.".to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "symbol": {
                        "type": "string",
                        "description": "The stock ticker symbol (e.g. AAPL, MSFT)."
                    }
                },
                "required": ["symbol"]
            }),
        },
    }
}

pub async fn execute(arguments: &str) -> Result<String> {
    let input: CompanyInfoInput = serde_json::from_str(arguments)?;
    let symbol = input.symbol.to_uppercase();

    let ticker = Ticker::new(&symbol).await?;
    let profile_opt = ticker.asset_profile().await?;

    let Some(profile) = profile_opt else {
        return Ok(format!("## {symbol} — Company Profile\n\nNo profile data available."));
    };

    let sector = profile.sector.as_deref().unwrap_or("N/A");
    let industry = profile.industry.as_deref().unwrap_or("N/A");
    let website = profile.website.as_deref().unwrap_or("N/A");
    let country = profile.country.as_deref().unwrap_or("N/A");
    let city = profile.city.as_deref().unwrap_or("");
    let state = profile.state.as_deref().unwrap_or("");

    let location = match (city.is_empty(), state.is_empty()) {
        (false, false) => format!("{city}, {state}, {country}"),
        (false, true) => format!("{city}, {country}"),
        _ => country.to_string(),
    };

    let employees = profile
        .full_time_employees
        .map(|n| format!("{}", n))
        .unwrap_or_else(|| "N/A".to_string());

    let phone = profile.phone.as_deref().unwrap_or("N/A");

    let mut output = format!(
        "## {symbol} — Company Profile\n\n\
        **Sector:** {sector} | **Industry:** {industry}\n\
        **Location:** {location}\n\
        **Employees:** {employees} | **Website:** {website} | **Phone:** {phone}\n\n"
    );

    // Governance risk scores
    let audit = profile.audit_risk.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string());
    let board = profile.board_risk.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string());
    let comp = profile.compensation_risk.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string());
    let shareholder = profile.shareholder_rights_risk.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string());
    let overall = profile.overall_risk.map(|v| v.to_string()).unwrap_or_else(|| "N/A".to_string());

    output.push_str(&format!(
        "### Governance Risk (1=low, 10=high)\n\
        **Audit:** {audit} | **Board:** {board} | **Compensation:** {comp} | **Shareholder Rights:** {shareholder} | **Overall:** {overall}\n\n"
    ));

    // Key officers (up to 5)
    if !profile.company_officers.is_empty() {
        output.push_str("### Key Officers\n\n");
        output.push_str("| Name | Title | Total Pay |\n");
        output.push_str("|------|-------|-----------|\n");

        for officer in profile.company_officers.iter().take(5) {
            let name = officer.name.as_deref().unwrap_or("N/A");
            let title = officer.title.as_deref().unwrap_or("N/A");
            let pay = officer
                .total_pay
                .as_ref()
                .and_then(|v| v.fmt.as_deref())
                .unwrap_or("N/A");
            output.push_str(&format!("| {} | {} | {} |\n", name, title, pay));
        }
        output.push('\n');
    }

    // Business summary
    if let Some(summary) = &profile.long_business_summary {
        let truncated = if summary.len() > 800 {
            format!("{}...", &summary[..800])
        } else {
            summary.clone()
        };
        output.push_str("### Business Summary\n\n");
        output.push_str(&truncated);
        output.push('\n');
    }

    Ok(output)
}
