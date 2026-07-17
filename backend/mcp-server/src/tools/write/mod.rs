//! Write tools, grouped by domain.
//!
//! The shared helpers live here because every write tool needs the same three things: a way
//! to surface a backend error, a "you cannot see that" error that does not confirm the id
//! exists, and a plain text result.

use rmcp::{ErrorData, model::*};

pub mod notebook;
pub mod playbook;
pub mod principle;
pub mod tags;

pub(crate) fn internal<E: std::fmt::Display>(e: E) -> ErrorData {
    ErrorData::internal_error(e.to_string(), None)
}

/// Deliberately does not distinguish "no such id" from "not yours": confirming that a
/// stranger's id exists is itself a leak, and every id here is resolved by (id, caller).
pub(crate) fn not_found(what: &str) -> ErrorData {
    ErrorData::invalid_params(format!("{what} not found"), None)
}

pub(crate) fn ok(message: impl Into<String>) -> Result<CallToolResult, ErrorData> {
    Ok(CallToolResult::success(vec![Content::text(message.into())]))
}

#[cfg(test)]
mod tests {
    use crate::server::TradstryMcp;

    /// Each domain lives in its own `#[tool_router]` block, merged in `server.rs`. A router
    /// that never gets merged still compiles — the tools would simply not exist.
    #[test]
    fn every_write_tool_is_registered() {
        let names: Vec<String> = (TradstryMcp::write_router()
            + TradstryMcp::playbook_write_router()
            + TradstryMcp::tags_write_router()
            + TradstryMcp::principle_write_router())
        .list_all()
        .into_iter()
        .map(|t| t.name.to_string())
        .collect();

        for expected in [
            "create_note",
            "update_note",
            "delete_note",
            "move_note",
            "create_folder",
            "create_playbook",
            "update_playbook",
            "delete_playbook",
            "set_trade_playbook",
            "create_tag",
            "tag_trade",
            "delete_tag",
            "merge_tags",
            "create_principle",
            "update_principle",
            "delete_principle",
            "flag_violation",
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn the_tags_read_tool_exists_so_write_tools_have_ids_to_use() {
        let names: Vec<String> = TradstryMcp::tags_read_router()
            .list_all()
            .into_iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(names.contains(&"list_tags".to_string()));
    }
}
