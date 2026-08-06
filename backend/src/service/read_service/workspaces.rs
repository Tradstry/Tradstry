use anyhow::Result;

use crate::service::db::client::UserDb;
use crate::service::db::schema::tables::workspaces_table::{
    self, CreateWorkspaceInput, UpdateWorkspaceInput, Workspace,
};

pub async fn list_workspaces(user_db: &UserDb) -> Result<Vec<Workspace>> {
    workspaces_table::list_workspaces(user_db.pool(), user_db.user_id()).await
}

pub async fn get_workspace(user_db: &UserDb, id: &str) -> Result<Option<Workspace>> {
    workspaces_table::find_workspace(user_db.pool(), id, user_db.user_id()).await
}

pub async fn create_workspace(user_db: &UserDb, input: CreateWorkspaceInput) -> Result<Workspace> {
    workspaces_table::create_workspace(user_db.pool(), user_db.user_id(), input).await
}

pub async fn update_workspace(
    user_db: &UserDb,
    id: &str,
    input: UpdateWorkspaceInput,
) -> Result<Workspace> {
    workspaces_table::update_workspace(user_db.pool(), id, user_db.user_id(), input).await
}

pub async fn delete_workspace(user_db: &UserDb, id: &str) -> Result<bool> {
    workspaces_table::delete_workspace(user_db.pool(), id, user_db.user_id()).await
}
