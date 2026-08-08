use anyhow::Result;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

use crate::service::ai::chat::types::{LlmFunctionDef, LlmToolDef};
use crate::service::db::Db;
use crate::service::read_service::playbook as playbook_service;

#[derive(Debug, Default, Deserialize)]
struct GetPlaybooksInput {
    playbook_id: Option<String>,
    playbook_name: Option<String>,
    workspace_id: String,
}

pub fn schema() -> LlmToolDef {
    LlmToolDef {
        tool_type: "function".to_string(),
        function: LlmFunctionDef {
            name: "get_playbook".to_string(),
            description:
                "List all of the user's trading playbooks with full details (edge, entry rules, \
                 exit rules, position-sizing rules, additional rules) plus performance stats \
                 (win rate, cumulative profit, average gain/loss, trade count). Optionally pass \
                 playbook_id to fetch a single playbook by id."
                    .to_string(),
            parameters: json!({
                "type": "object",
                "required": ["workspace_id"],
                "properties": {
                    "playbook_id": {
                        "type": "string",
                        "description": "Optional playbook id. When provided, returns that single playbook; otherwise returns all playbooks."
                    },
                    "playbook_name": {
                        "type": "string",
                        "description": "Optional exact playbook name when its internal id is not known."
                    },
                    "workspace_id": {
                        "type": "string",
                        "description": "Workspace whose playbooks should be listed."
                    }
                }
            }),
        },
    }
}

pub async fn execute(arguments: &str, user_id: &str, db: &Arc<Db>) -> Result<String> {
    let input: GetPlaybooksInput = serde_json::from_str(arguments)?;
    let user_db = db.get_user_db(user_id);

    match input.playbook_id {
        Some(id) => match playbook_service::get_playbook(&user_db, &id).await? {
            Some(playbook) if playbook.workspace_id == input.workspace_id => {
                Ok(serde_json::to_string(&playbook)?)
            }
            None => Ok("Playbook not found.".to_string()),
            Some(_) => Ok("Playbook not found in this workspace.".to_string()),
        },
        None => {
            let mut playbooks =
                playbook_service::list_playbooks(&user_db, &input.workspace_id).await?;
            if let Some(name) = input.playbook_name {
                playbooks.retain(|playbook| playbook.name.eq_ignore_ascii_case(name.trim()));
            }
            Ok(serde_json::to_string(&playbooks)?)
        }
    }
}
