//! Write tools for the notebook: an agent composes Markdown, these turn it into notes.
//!
//! A note's body is a Lexical document, and once a client has opened the note it is backed
//! by a Yjs CRDT — at which point `document_json` is no longer authoritative and writes to
//! it are rejected outright. So "write text into a note" is two different operations
//! depending on the note's state, and neither one is a plain UPDATE.
//!
//! Both conversions live in `projector/markdown.mjs`, not here: it is the same bundle that
//! seeds and projects notes, sharing `@tradstry/notebook-core` with the desktop. A Rust-side
//! idea of the document format would be a second implementation, and the two would drift.
//!
//! The rule this file exists to enforce: editing a CRDT note means appending an *incremental*
//! Yjs update built on that note's existing history. A document rebuilt from scratch does not
//! conflict with the live one — it concatenates, silently doubling every paragraph.

use rmcp::{
    ErrorData, RoleServer, handler::server::wrapper::Parameters, model::*, service::RequestContext,
    tool, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use tradstry_backend::graphql::notebook::crdt as crdt_api;
use tradstry_backend::graphql::notebook::sync;
use tradstry_backend::service::ai::projector::{self, EditMode};
use tradstry_backend::service::db::schema::tables::notebook::{
    crdt, folders,
    notes::{self, CreateNotebookNoteInput, UpdateNotebookNoteInput},
};

use crate::server::TradstryMcp;
use crate::tools::write::{internal, ok};

#[derive(Debug, Deserialize, Serialize, JsonSchema, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WriteMode {
    /// Swap the note's whole body for this markdown.
    Replace,
    /// Add this markdown to the end, leaving what is already there untouched.
    Append,
}

impl From<WriteMode> for EditMode {
    fn from(m: WriteMode) -> Self {
        match m {
            WriteMode::Replace => EditMode::Replace,
            WriteMode::Append => EditMode::Append,
        }
    }
}

/// Parameters for `create_note`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateNoteParams {
    /// Workspace to create the note under. Call `list_workspaces` first.
    pub workspace_id: String,
    /// The note body as Markdown. Headings, lists, code blocks, quotes, links, bold and
    /// italic all convert. Begin with an `# H1` — the note's title is taken from the first
    /// H1, so a note without one ends up untitled.
    pub markdown: String,
    /// Folder to file the note in. Defaults to the account's System folder, which exists for
    /// exactly this purpose. Pass a folder id to put it somewhere else.
    pub folder_id: Option<String>,
}

/// Parameters for `update_note`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct UpdateNoteParams {
    /// The note to edit. Obtain ids from `get_notebook`.
    pub note_id: String,
    /// Markdown to write. Under `replace` this becomes the entire body, so send the whole
    /// note, not a fragment. Under `append` it is added to the end.
    pub markdown: String,
    /// `replace` swaps the whole body; `append` adds to the end. Prefer `append` when adding
    /// to a running log — it cannot destroy existing content.
    pub mode: WriteMode,
}

/// Parameters for `delete_note`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct DeleteNoteParams {
    /// The note to delete.
    pub note_id: String,
}

/// Parameters for `move_note`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct MoveNoteParams {
    /// The note to move.
    pub note_id: String,
    /// Destination folder. Omit to move the note out of any folder (Uncategorized).
    pub folder_id: Option<String>,
}

/// Parameters for `create_folder`.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct CreateFolderParams {
    /// Workspace to create the folder under.
    pub workspace_id: String,
    /// Folder name.
    pub name: String,
    /// Optional parent folder, to nest it.
    pub parent_folder_id: Option<String>,
}

#[tool_router(router = write_router, vis = "pub")]
impl TradstryMcp {
    #[tool(
        description = "Create a notebook note from Markdown. Use this to file a report or \
                       analysis the user can read later. The body is Markdown (headings, \
                       lists, code, quotes, links, bold/italic); begin with an `# H1`, which \
                       becomes the note's title. Defaults to the account's System folder — the \
                       folder that exists to hold agent-written notes. Returns the note id."
    )]
    pub async fn create_note(
        &self,
        Parameters(params): Parameters<CreateNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        if params.markdown.trim().is_empty() {
            return Err(ErrorData::invalid_params("markdown is empty", None));
        }

        // Fetch the account's folders once: to validate a caller-supplied folder_id (an
        // agent can hallucinate an id) and, absent one, to file the note in System.
        let account_folders = folders::list_notebook_folders(user_db.pool(), &params.workspace_id)
            .await
            .map_err(internal)?;
        let folder_id = match params.folder_id {
            Some(id) => {
                if !account_folders.iter().any(|f| f.id == id) {
                    return Err(ErrorData::invalid_params(
                        format!("folder_id '{id}' does not exist in this account"),
                        None,
                    ));
                }
                Some(id)
            }
            None => account_folders
                .iter()
                .find(|f| f.is_system)
                .map(|f| f.id.clone()),
        };

        let document_json = projector::markdown_to_json(&params.markdown)
            .await
            .map_err(internal)?;

        let note = notes::create_notebook_note(
            user_db.pool(),
            user_db.user_id(),
            CreateNotebookNoteInput {
                id: None,
                workspace_id: params.workspace_id,
                document_json,
                trade_ids: Vec::new(),
                folder_id,
            },
        )
        .await
        .map_err(internal)?;

        // Seed the note into a Y.Doc now, exactly as the web create resolver does. Without
        // this the note stays `legacy` with zero CRDT updates, so the editor shows
        // "Syncing…" forever — and the sweeper only re-drives `seeding`, never `legacy`,
        // so nothing ever rescues it.
        sync::seed_new_note(user_db.pool(), &note.id).await;

        ok(format!(
            "Created note {} titled \"{}\".",
            note.id, note.title
        ))
    }

    #[tool(
        description = "Rewrite or extend an existing note's body with Markdown. mode=\"append\" \
                       adds to the end and cannot destroy existing content; mode=\"replace\" \
                       swaps the entire body, so send the complete note, not a fragment. Safe \
                       to call while the user has the note open — the change merges with their \
                       edits rather than overwriting them."
    )]
    pub async fn update_note(
        &self,
        Parameters(params): Parameters<UpdateNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;
        let pool = user_db.pool();

        if params.markdown.trim().is_empty() {
            return Err(ErrorData::invalid_params("markdown is empty", None));
        }

        // Ownership: a foreign note id must not be writable — or even confirmable.
        let note = notes::find_notebook_note(pool, &params.note_id, user_db.user_id())
            .await
            .map_err(internal)?
            .ok_or_else(|| ErrorData::invalid_params("note not found", None))?;

        match crdt::note_state(pool, &note.id).await.map_err(internal)? {
            // No client has opened it, so `document_json` is still authoritative and a plain
            // body write is correct.
            crdt::NoteState::Legacy => {
                let document_json = match params.mode {
                    WriteMode::Replace => projector::markdown_to_json(&params.markdown)
                        .await
                        .map_err(internal)?,
                    // Structural, not a markdown round-trip: that would drop images and
                    // linked trades, which markdown cannot express.
                    WriteMode::Append => {
                        projector::append_markdown_to_json(&note.document_json, &params.markdown)
                            .await
                            .map_err(internal)?
                    }
                };

                notes::update_notebook_note(
                    pool,
                    &note.id,
                    user_db.user_id(),
                    UpdateNotebookNoteInput {
                        workspace_id: None,
                        document_json: Some(document_json),
                        trade_ids: None,
                        folder_id: None,
                        expected_updated_at: None,
                    },
                )
                .await
                .map_err(internal)?;
            }

            // The seed subprocess is in flight. Writing either representation now races it,
            // so refuse and let the caller retry rather than corrupt the note.
            crdt::NoteState::Seeding => {
                return Err(internal(
                    "note is being prepared for collaborative editing; retry in a moment",
                ));
            }

            // The update chain is authoritative. Build a delta on top of that history and
            // append it — never rewrite `document_json`, which clients no longer read.
            crdt::NoteState::Crdt => {
                let history: Vec<Vec<u8>> =
                    crdt_api::updates_since(pool, user_db.user_id(), &note.id, 0)
                        .await
                        .map_err(internal)?
                        .into_iter()
                        .map(|(_, bytes)| bytes)
                        .collect();

                let update =
                    projector::apply_markdown(&history, &params.markdown, params.mode.into())
                        .await
                        .map_err(internal)?;

                crdt_api::append_updates(pool, user_db.user_id(), &note.id, &[update])
                    .await
                    .map_err(internal)?;

                // Keep `document_json` — which search, previews and the MCP read tools all
                // use — in step with the chain we just extended. Without this the note reads
                // as stale everywhere except the editor.
                crdt::refresh_projection(pool, &note.id)
                    .await
                    .map_err(internal)?;
            }
        }

        ok(format!(
            "Updated note {} ({}).",
            note.id,
            match params.mode {
                WriteMode::Replace => "replaced",
                WriteMode::Append => "appended",
            }
        ))
    }

    #[tool(
        description = "Delete a notebook note. Soft delete: it disappears from every device. \
                       Notes inside the System folder are ordinary notes and delete freely; \
                       the System folder itself cannot be deleted."
    )]
    pub async fn delete_note(
        &self,
        Parameters(params): Parameters<DeleteNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let deleted =
            notes::delete_notebook_note(user_db.pool(), &params.note_id, user_db.user_id())
                .await
                .map_err(internal)?;

        if !deleted {
            return Err(ErrorData::invalid_params("note not found", None));
        }
        ok(format!("Deleted note {}.", params.note_id))
    }

    #[tool(
        description = "Move a note into a folder, or out of every folder (Uncategorized) by \
                       omitting folder_id."
    )]
    pub async fn move_note(
        &self,
        Parameters(params): Parameters<MoveNoteParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let note = notes::find_notebook_note(user_db.pool(), &params.note_id, user_db.user_id())
            .await
            .map_err(internal)?
            .ok_or_else(|| ErrorData::invalid_params("note not found", None))?;

        notes::update_notebook_note(
            user_db.pool(),
            &note.id,
            user_db.user_id(),
            UpdateNotebookNoteInput {
                workspace_id: None,
                document_json: None,
                trade_ids: None,
                folder_id: params.folder_id.clone(),
                expected_updated_at: None,
            },
        )
        .await
        .map_err(internal)?;

        ok(match params.folder_id {
            Some(f) => format!("Moved note {} to folder {f}.", note.id),
            None => format!("Moved note {} out of its folder.", note.id),
        })
    }

    #[tool(description = "Create a notebook folder. Returns the folder id.")]
    pub async fn create_folder(
        &self,
        Parameters(params): Parameters<CreateFolderParams>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        let u = self.user(&ctx)?;
        let user_db = self.synced_user_db(&u.user_id).await?;

        let name = params.name.trim();
        if name.is_empty() {
            return Err(ErrorData::invalid_params("name is empty", None));
        }

        // A supplied parent must exist in this account, or the folder would be orphaned or
        // silently mis-nested under an id from somewhere else.
        if let Some(parent_id) = &params.parent_folder_id {
            let account_folders =
                folders::list_notebook_folders(user_db.pool(), &params.workspace_id)
                    .await
                    .map_err(internal)?;
            if !account_folders.iter().any(|f| f.id == *parent_id) {
                return Err(ErrorData::invalid_params(
                    format!("parent_folder_id '{parent_id}' does not exist in this account"),
                    None,
                ));
            }
        }

        let folder = folders::create_notebook_folder(
            user_db.pool(),
            folders::CreateNotebookFolderInput {
                id: None,
                user_id: user_db.user_id().to_string(),
                workspace_id: params.workspace_id,
                parent_folder_id: params.parent_folder_id,
                name: name.to_string(),
            },
        )
        .await
        .map_err(internal)?;

        ok(format!(
            "Created folder {} named \"{}\".",
            folder.id, folder.name
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The write tools live in a second `#[tool_router]` block that `server.rs` merges into
    /// the main one. A failed merge still compiles — the tools would simply never be exposed.
    #[test]
    fn the_write_tools_are_registered() {
        let names: Vec<String> = TradstryMcp::write_router()
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
        ] {
            assert!(names.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn update_mode_serializes_to_the_words_the_tool_description_promises() {
        assert_eq!(
            serde_json::to_string(&WriteMode::Append).unwrap(),
            "\"append\""
        );
        assert_eq!(
            serde_json::to_string(&WriteMode::Replace).unwrap(),
            "\"replace\""
        );
    }
}
