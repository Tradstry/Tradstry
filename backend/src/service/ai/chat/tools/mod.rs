pub mod analytics_calc;
pub mod company_info;
pub mod create_agent;
pub mod db_query;
pub mod earnings;
pub mod edit_agent;
pub mod financials;
pub mod notebook;
pub mod playbook;
pub mod recall_memory;
pub mod run_agent;
pub mod save_agent;
pub mod semantic_search;
pub mod stock_news;
pub mod stock_quote;
pub mod view_media;

use crate::service::ai::client::AgentsClient;
use crate::service::ai::vector_database::client::VectorDatabaseClient;
use crate::service::db::Db;
use crate::service::r2::R2Client;
use anyhow::Result;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRoute {
    DirectAnswer,
    Records,
    SemanticSearch,
    Analytics,
    Research,
    Report,
    Comparison,
    Notebook,
    Media,
    Playbook,
    Memory,
    AgentCreateStart,
    AgentCreateContinue,
    AgentRun,
    AgentEdit,
    StockQuote,
    StockNews,
    Earnings,
    Financials,
    CompanyInfo,
}

impl ToolRoute {
    pub fn label(self) -> &'static str {
        match self {
            Self::DirectAnswer => "direct_answer",
            Self::Records => "records",
            Self::SemanticSearch => "semantic_search",
            Self::Analytics => "analytics",
            Self::Research => "research",
            Self::Report => "report",
            Self::Comparison => "comparison",
            Self::Notebook => "notebook",
            Self::Media => "media",
            Self::Playbook => "playbook",
            Self::Memory => "memory",
            Self::AgentCreateStart => "agent_create_start",
            Self::AgentCreateContinue => "agent_create_continue",
            Self::AgentRun => "agent_run",
            Self::AgentEdit => "agent_edit",
            Self::StockQuote => "stock_quote",
            Self::StockNews => "stock_news",
            Self::Earnings => "earnings",
            Self::Financials => "financials",
            Self::CompanyInfo => "company_info",
        }
    }

    pub fn max_tool_calls(self) -> u32 {
        match self {
            Self::DirectAnswer => 0,
            Self::Media => 2,
            _ => 1,
        }
    }

    pub fn requires_tool_at(self, iteration: u32) -> bool {
        match self {
            Self::DirectAnswer | Self::AgentCreateContinue => false,
            Self::Media => iteration < 2,
            _ => iteration == 0,
        }
    }

    fn allowed_names(self) -> &'static [&'static str] {
        match self {
            Self::DirectAnswer => &[],
            Self::Records => &["db_query"],
            Self::SemanticSearch => &["semantic_search"],
            Self::Analytics => &["analytics_calc"],
            Self::Research => &["research"],
            Self::Report => &["report"],
            Self::Comparison => &["comparison"],
            Self::Notebook => &["get_notebook"],
            Self::Media => &["get_notebook", "view_media"],
            Self::Playbook => &["get_playbook"],
            Self::Memory => &["recall_memory"],
            Self::AgentCreateStart => &["create_agent"],
            Self::AgentCreateContinue => &["save_agent"],
            Self::AgentRun => &["run_agent"],
            Self::AgentEdit => &["edit_agent"],
            Self::StockQuote => &["stock_quote"],
            Self::StockNews => &["stock_news"],
            Self::Earnings => &["earnings"],
            Self::Financials => &["financials"],
            Self::CompanyInfo => &["company_info"],
        }
    }
}

fn contains_any(text: &str, phrases: &[&str]) -> bool {
    phrases.iter().any(|phrase| text.contains(phrase))
}

fn latest_user_text(messages: &[Value]) -> String {
    messages
        .iter()
        .rev()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|message| message.get("content"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_lowercase()
}

fn has_recent_agent_context(messages: &[Value]) -> bool {
    messages.iter().rev().take(8).any(|message| {
        message
            .get("tool_calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| {
                calls.iter().any(|call| {
                    call.get("function")
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                        .is_some_and(|name| {
                            matches!(
                                name,
                                "create_agent" | "save_agent" | "run_agent" | "edit_agent"
                            )
                        })
                })
            })
    })
}

/// Pick one bounded tool family from the user's actual task. Specific workflows
/// win over broad data keywords so a request for a report or comparison cannot
/// wander through unrelated leaf tools first.
pub fn route_for_messages(messages: &[Value]) -> ToolRoute {
    let text = latest_user_text(messages);

    if contains_any(
        &text,
        &["run my agent", "run the agent", "execute my agent"],
    ) {
        return ToolRoute::AgentRun;
    }
    if contains_any(
        &text,
        &[
            "edit my agent",
            "change my agent",
            "rename my agent",
            "update my agent",
        ],
    ) {
        return ToolRoute::AgentEdit;
    }
    if contains_any(
        &text,
        &[
            "create an agent",
            "create agent",
            "build an agent",
            "automated workflow",
        ],
    ) {
        return ToolRoute::AgentCreateStart;
    }
    if text.split_whitespace().count() <= 8 && has_recent_agent_context(messages) {
        return ToolRoute::AgentCreateContinue;
    }
    if contains_any(
        &text,
        &[
            "compare",
            "comparison",
            " versus ",
            " vs ",
            "difference between",
        ],
    ) {
        return ToolRoute::Comparison;
    }
    if contains_any(
        &text,
        &[
            "performance report",
            "trading report",
            "weekly report",
            "monthly report",
            "weekly review",
            "monthly review",
            "full report",
        ],
    ) {
        return ToolRoute::Report;
    }
    if contains_any(
        &text,
        &[
            "deep dive",
            "recurring pattern",
            "recurring mistake",
            "recurring setup",
            "patterns in",
            "trading patterns",
            "common mistakes",
            "actionable insights",
            "what patterns",
            "look for patterns",
            "analyze my trades",
            "analyze my trading",
            "analyze my recent",
            "analyse my trades",
            "analyse my trading",
            "analyse my recent",
            "analyze my losses",
            "analyse my losses",
            "review my trades",
            "why do i keep",
        ],
    ) {
        return ToolRoute::Research;
    }
    if contains_any(
        &text,
        &["screenshot", "image", "video", "media", "chart attachment"],
    ) {
        return ToolRoute::Media;
    }
    if contains_any(
        &text,
        &["notebook", "my note", "journal note", "trading note"],
    ) {
        return ToolRoute::Notebook;
    }
    if contains_any(
        &text,
        &[
            "playbook",
            "entry rules",
            "exit rules",
            "setup rules",
            "position sizing rules",
        ],
    ) {
        return ToolRoute::Playbook;
    }
    if contains_any(
        &text,
        &[
            "do you remember",
            "what did i say",
            "what have i told you",
            "we discussed",
            "we talked about",
            "what is my name",
        ],
    ) {
        return ToolRoute::Memory;
    }
    if contains_any(
        &text,
        &[
            "latest news",
            "stock news",
            "market news",
            "headline",
            "catalyst",
        ],
    ) {
        return ToolRoute::StockNews;
    }
    if contains_any(
        &text,
        &[
            "earnings",
            "eps estimate",
            "revenue estimate",
            "earnings date",
        ],
    ) {
        return ToolRoute::Earnings;
    }
    if contains_any(
        &text,
        &[
            "financials",
            "income statement",
            "balance sheet",
            "cash flow",
            "financial statement",
        ],
    ) {
        return ToolRoute::Financials;
    }
    if contains_any(
        &text,
        &[
            "company profile",
            "what does the company",
            "what does this company",
            "sector",
            "industry",
        ],
    ) {
        return ToolRoute::CompanyInfo;
    }
    if contains_any(
        &text,
        &[
            "stock price",
            "share price",
            "quote",
            "market cap",
            "52-week",
            "52 week",
            "dividend yield",
            " beta",
        ],
    ) {
        return ToolRoute::StockQuote;
    }
    if contains_any(
        &text,
        &[
            "win rate",
            "p&l",
            "pnl",
            "profit factor",
            "average r",
            "avg r",
            "expectancy",
            "drawdown",
            "streak",
            "performance metrics",
            "how many trades",
            "trade count",
            "how am i doing",
            "my edge",
        ],
    ) {
        return ToolRoute::Analytics;
    }
    if contains_any(
        &text,
        &[
            "trades where",
            "notes about",
            "journal about",
            "held too long",
            "felt anxious",
            "felt emotional",
            "by meaning",
        ],
    ) {
        return ToolRoute::SemanticSearch;
    }
    if contains_any(
        &text,
        &[
            "show my trade",
            "show me my trade",
            "list my trade",
            "find my trade",
            "get my trade",
            "my journal entr",
            "trade details",
        ],
    ) {
        return ToolRoute::Records;
    }
    if contains_any(
        &text,
        &[
            "my trades",
            "my trading",
            "my journal",
            "my results",
            "my performance",
        ],
    ) {
        return ToolRoute::Research;
    }

    ToolRoute::DirectAnswer
}

pub fn route_for_turn(
    messages: &[Value],
    has_pinned_trades: bool,
    has_pinned_playbooks: bool,
    has_pinned_date_range: bool,
) -> ToolRoute {
    let route = route_for_messages(messages);
    if has_pinned_trades
        && matches!(
            route,
            ToolRoute::DirectAnswer
                | ToolRoute::Records
                | ToolRoute::SemanticSearch
                | ToolRoute::Analytics
                | ToolRoute::Research
                | ToolRoute::Report
        )
    {
        return ToolRoute::Records;
    }

    if route == ToolRoute::DirectAnswer && has_pinned_playbooks {
        return ToolRoute::Playbook;
    }

    if route == ToolRoute::DirectAnswer && has_pinned_date_range {
        return ToolRoute::Research;
    }

    route
}

pub fn tool_schemas_for_route(
    route: ToolRoute,
    iteration: u32,
) -> Vec<crate::service::ai::chat::types::LlmToolDef> {
    let allowed: &[&str] = if route == ToolRoute::Media {
        if iteration == 0 {
            &["get_notebook"]
        } else {
            &["view_media"]
        }
    } else {
        route.allowed_names()
    };
    tool_schemas()
        .into_iter()
        .filter(|tool| allowed.contains(&tool.function.name.as_str()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn execute_tool(
    name: &str,
    arguments: &str,
    user_id: &str,
    workspace_id: &str,
    db: &Arc<Db>,
    qdrant: &Arc<VectorDatabaseClient>,
    r2: &Arc<R2Client>,
    agents: Option<&Arc<AgentsClient>>,
    _checkpoint_saver: Option<&Arc<dyn langgraph::prelude::CheckpointSaver>>,
    conversation_messages: Option<&serde_json::Value>,
) -> Result<String> {
    match name {
        "db_query" => db_query::execute(arguments, user_id, workspace_id, db).await,
        "semantic_search" => {
            semantic_search::execute(arguments, user_id, workspace_id, qdrant).await
        }
        "analytics_calc" => analytics_calc::execute(arguments, user_id, workspace_id, db).await,
        "get_notebook" => notebook::execute(arguments, user_id, db).await,
        "view_media" => {
            let agents =
                agents.ok_or_else(|| anyhow::anyhow!("AgentsClient unavailable for view_media"))?;
            view_media::execute(arguments, user_id, r2, db, agents).await
        }
        "get_playbook" => playbook::execute(arguments, user_id, db).await,
        "recall_memory" => {
            recall_memory::execute(arguments, user_id, qdrant, conversation_messages).await
        }
        "create_agent" => create_agent::execute(arguments),
        "save_agent" => save_agent::execute(arguments, user_id, workspace_id, db).await,
        "run_agent" => {
            let agents = agents
                .ok_or_else(|| anyhow::anyhow!("AgentsClient not available for run_agent"))?;
            run_agent::execute(arguments, user_id, workspace_id, agents, db, qdrant, r2).await
        }
        "edit_agent" => edit_agent::execute(arguments, user_id, workspace_id, db).await,
        "stock_quote" => stock_quote::execute(arguments).await,
        "stock_news" => stock_news::execute(arguments).await,
        "financials" => financials::execute(arguments).await,
        "earnings" => earnings::execute(arguments).await,
        "company_info" => company_info::execute(arguments).await,
        _ => Ok(format!("Unknown tool: {}", name)),
    }
}

pub fn tool_schemas() -> Vec<crate::service::ai::chat::types::LlmToolDef> {
    vec![
        db_query::schema(),
        semantic_search::schema(),
        analytics_calc::schema(),
        notebook::schema(),
        view_media::schema(),
        playbook::schema(),
        recall_memory::schema(),
        create_agent::schema(),
        save_agent::schema(),
        run_agent::schema(),
        edit_agent::schema(),
        stock_quote::schema(),
        stock_news::schema(),
        financials::schema(),
        earnings::schema(),
        company_info::schema(),
        crate::service::ai::chat::subgraphs::research::tool_schema(),
        crate::service::ai::chat::subgraphs::report::tool_schema(),
        crate::service::ai::chat::subgraphs::comparison::tool_schema(),
    ]
}

#[cfg(test)]
mod tests {
    use super::{ToolRoute, route_for_messages, route_for_turn, tool_schemas_for_route};
    use serde_json::json;

    fn route(message: &str) -> ToolRoute {
        route_for_messages(&[json!({"role": "user", "content": message})])
    }

    #[test]
    fn routes_deep_analysis_to_single_research_pipeline() {
        assert_eq!(
            route("Analyze my trades and find recurring patterns in my losses"),
            ToolRoute::Research
        );
        let names = tool_schemas_for_route(ToolRoute::Research, 0)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["research"]);
    }

    #[test]
    fn routes_metrics_without_exposing_research_or_records() {
        assert_eq!(
            route("What is my win rate, profit factor, and average R?"),
            ToolRoute::Analytics
        );
        let names = tool_schemas_for_route(ToolRoute::Analytics, 0)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["analytics_calc"]);
        assert_eq!(route("How many trades have I made?"), ToolRoute::Analytics);
    }

    #[test]
    fn routes_reports_comparisons_and_notebook_tasks_separately() {
        assert_eq!(route("Give me a weekly report"), ToolRoute::Report);
        assert_eq!(route("Compare these two trades"), ToolRoute::Comparison);
        assert_eq!(
            route("Analyze the screenshot in my notebook"),
            ToolRoute::Media
        );
    }

    #[test]
    fn media_route_enforces_notebook_then_media_order() {
        let first = tool_schemas_for_route(ToolRoute::Media, 0)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect::<Vec<_>>();
        let second = tool_schemas_for_route(ToolRoute::Media, 1)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect::<Vec<_>>();
        assert_eq!(first, vec!["get_notebook"]);
        assert_eq!(second, vec!["view_media"]);
        assert!(ToolRoute::Media.requires_tool_at(0));
        assert!(ToolRoute::Media.requires_tool_at(1));
    }

    #[test]
    fn routes_the_actual_recent_patterns_starter_to_research() {
        assert_eq!(
            route(
                "Analyze my recent trading patterns. Look for recurring setups, common mistakes, and actionable insights."
            ),
            ToolRoute::Research
        );
    }

    #[test]
    fn simple_knowledge_question_uses_no_tools() {
        assert_eq!(
            route("What does risk reward ratio mean?"),
            ToolRoute::DirectAnswer
        );
        assert_eq!(
            route("Remember that I prefer futures"),
            ToolRoute::DirectAnswer
        );
        assert_eq!(
            route("Do you remember what I said about futures?"),
            ToolRoute::Memory
        );
    }

    #[test]
    fn ambiguous_request_with_pinned_trade_uses_exact_records() {
        let messages = [json!({"role": "user", "content": "Analyze these for me"})];
        assert_eq!(
            route_for_turn(&messages, true, false, false),
            ToolRoute::Records
        );
        assert_eq!(
            route_for_turn(&messages, false, false, false),
            ToolRoute::DirectAnswer
        );
    }

    #[test]
    fn pinned_trade_overrides_broad_journal_analysis_but_not_market_news() {
        assert_eq!(
            route_for_turn(
                &[json!({"role": "user", "content": "Analyze my trades"})],
                true,
                false,
                false,
            ),
            ToolRoute::Records
        );
        assert_eq!(
            route_for_turn(
                &[json!({"role": "user", "content": "Show me the latest news"})],
                true,
                false,
                false,
            ),
            ToolRoute::StockNews
        );
    }

    #[test]
    fn short_agent_followup_keeps_agent_creation_route() {
        let messages = vec![
            json!({"role": "user", "content": "Create an agent for morning reviews"}),
            json!({
                "role": "assistant",
                "tool_calls": [{"function": {"name": "create_agent", "arguments": "{}"}}]
            }),
            json!({"role": "tool", "content": "What should we name this agent?"}),
            json!({"role": "assistant", "content": "What should we name it?"}),
            json!({"role": "user", "content": "Morning Edge"}),
        ];

        assert_eq!(
            route_for_messages(&messages),
            ToolRoute::AgentCreateContinue
        );
        let names = tool_schemas_for_route(ToolRoute::AgentCreateContinue, 0)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["save_agent"]);
    }

    #[test]
    fn initial_agent_request_only_exposes_create_agent() {
        assert_eq!(
            route("Create an agent for morning reviews"),
            ToolRoute::AgentCreateStart
        );
        let names = tool_schemas_for_route(ToolRoute::AgentCreateStart, 0)
            .into_iter()
            .map(|tool| tool.function.name)
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["create_agent"]);
    }
}
