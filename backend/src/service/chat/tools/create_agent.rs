use anyhow::Result;
use serde::Deserialize;
use serde_json::json;

use crate::service::chat::types::{LlmFunctionDef, LlmToolDef};

#[derive(Debug, Deserialize)]
struct CreateAgentInput {
    initial_description: String,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "create_agent".to_string(),
            description:
                "Start creating a custom agent. Use when the user wants to build a reusable \
                 automated workflow."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "initial_description": {
                        "type": "string",
                        "description": "The user's initial description of what they want the agent to do."
                    }
                },
                "required": ["initial_description"]
            }),
        },
    }
}

pub fn execute(arguments: &str) -> Result<String> {
    let input: CreateAgentInput = serde_json::from_str(arguments).unwrap_or(CreateAgentInput {
        initial_description: String::new(),
    });

    let prompt = format!(
        r#"AGENT CREATION MODE. User wants: "{}"

YOU MUST ASK EXACTLY ONE QUESTION. DO NOT ASK MULTIPLE QUESTIONS. DO NOT LIST ALL STEPS.

Ask this question now:

"What should we name this agent? Pick one:"

A) [generate a 2-3 word name based on their description]
B) [generate an alternative 2-3 word name]
C) [generate a third option]
D) Type your own name

STOP HERE. Do not ask about goal, data sources, or anything else yet. Wait for their answer. After they pick a name, you will ask the next question in your NEXT response (goal, then data sources, then symbol, then output style - one at a time, each with A/B/C options). When you have all answers, call save_agent."#,
        input.initial_description
    );

    Ok(prompt)
}
