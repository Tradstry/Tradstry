use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::service::db::schema::tables::user_agents_table::UserAgent;

// ---------------------------------------------------------------------------
// AgentStep
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStep {
    pub tool: String,
    pub args: Value,
    pub output_channel: Option<String>,
}

// ---------------------------------------------------------------------------
// AgentDefinition
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub goal: String,
    pub steps: Vec<AgentStep>,
    pub output_style: String,
    pub user_id: String,
    pub workspace_id: String,
}

impl AgentDefinition {
    /// Parse an `AgentDefinition` from the database `UserAgent` row.
    pub fn from_db(agent: &UserAgent) -> Result<Self> {
        let steps: Vec<AgentStep> = serde_json::from_str(&agent.steps_json)
            .with_context(|| format!("Failed to parse steps_json for agent '{}'", agent.id))?;

        Ok(Self {
            goal: agent.goal.clone(),
            steps,
            output_style: agent.output_style.clone(),
            user_id: agent.user_id.clone(),
            workspace_id: agent.workspace_id.clone(),
        })
    }

    /// Apply runtime parameter overrides to step args.
    ///
    /// Supported override keys: `symbol`, `date_from`, `date_to`, `query`.
    /// Filters are placed where each tool schema actually expects them.
    pub fn with_overrides(mut self, overrides: &Value) -> Self {
        for step in self.steps.iter_mut() {
            let Some(args) = step.args.as_object_mut() else {
                continue;
            };
            match step.tool.as_str() {
                "db_query" | "analytics_calc" => {
                    let filters = args
                        .entry("filters")
                        .or_insert_with(|| Value::Object(Default::default()));
                    if let Some(filters) = filters.as_object_mut() {
                        for key in ["symbol", "date_from", "date_to"] {
                            if let Some(value) = overrides.get(key) {
                                filters.insert(key.to_owned(), value.clone());
                            }
                        }
                    }
                }
                "semantic_search" => {
                    for key in ["date_from", "date_to", "query"] {
                        if let Some(value) = overrides.get(key) {
                            args.insert(key.to_owned(), value.clone());
                        }
                    }
                    if let Some(symbol) = overrides.get("symbol").and_then(Value::as_str) {
                        let query = args.get("query").and_then(Value::as_str).unwrap_or("");
                        args.insert(
                            "query".to_owned(),
                            Value::String(format!("{query} {symbol}")),
                        );
                    }
                }
                "stock_quote" | "stock_news" | "earnings" | "financials" | "company_info" => {
                    if let Some(value) = overrides.get("symbol") {
                        args.insert("symbol".to_owned(), value.clone());
                    }
                }
                _ => {}
            }
        }

        self
    }
}

#[cfg(test)]
mod tests {
    use super::{AgentDefinition, AgentStep};
    use serde_json::json;

    fn definition(tool: &str, args: serde_json::Value) -> AgentDefinition {
        AgentDefinition {
            goal: "test".to_owned(),
            steps: vec![AgentStep {
                tool: tool.to_owned(),
                args,
                output_channel: Some("result".to_owned()),
            }],
            output_style: "concise".to_owned(),
            user_id: "user".to_owned(),
            workspace_id: "workspace".to_owned(),
        }
    }

    #[test]
    fn puts_record_overrides_inside_filters() {
        let def = definition("analytics_calc", json!({"metrics":["win_rate"]}))
            .with_overrides(&json!({"symbol":"NVDA","date_from":"2026-08-01"}));

        assert_eq!(def.steps[0].args["filters"]["symbol"], "NVDA");
        assert_eq!(def.steps[0].args["filters"]["date_from"], "2026-08-01");
        assert!(def.steps[0].args.get("symbol").is_none());
    }

    #[test]
    fn puts_market_symbol_at_tool_root() {
        let def = definition("stock_quote", json!({})).with_overrides(&json!({"symbol":"AAPL"}));
        assert_eq!(def.steps[0].args["symbol"], "AAPL");
    }
}
