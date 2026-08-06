use anyhow::{Context, Result};
use sqlx::PgPool;

use super::client::SnapTradeAccount;
use crate::service::db::schema::tables::workspaces_table::{self, Workspace};

/// Bind one upstream SnapTrade account to one workspace. A brokerage
/// authorization can expose several upstream accounts; the workspace keeps its
/// existing binding, or selects the first account belonging to this connection.
/// It never creates additional workspaces implicitly.
pub async fn bind_workspace_brokerage_account(
    pool: &PgPool,
    user_id: &str,
    workspace_id: &str,
    snaptrade_accounts: &[SnapTradeAccount],
) -> Result<Vec<Workspace>> {
    let workspace = workspaces_table::find_workspace(pool, workspace_id, user_id)
        .await?
        .context("Workspace not found")?;

    let connection_id = workspace.snaptrade_connection_id.as_deref();
    if let Some(bound_id) = workspace.snaptrade_account_id.as_deref()
        && snaptrade_accounts.iter().any(|candidate| {
            candidate.id.as_deref() == Some(bound_id)
                && connection_id.is_none_or(|connection_id| {
                    candidate.brokerage_authorization.as_deref() == Some(connection_id)
                })
        })
    {
        return Ok(vec![workspace]);
    }

    let selected = snaptrade_accounts.iter().find(|candidate| {
        candidate.id.is_some()
            && connection_id.is_none_or(|connection_id| {
                candidate.brokerage_authorization.as_deref() == Some(connection_id)
            })
    });

    let Some(snaptrade_account_id) = selected.and_then(|candidate| candidate.id.as_deref()) else {
        return Ok(Vec::new());
    };

    let workspace = workspaces_table::set_snaptrade_account_id(
        pool,
        workspace_id,
        user_id,
        snaptrade_account_id,
    )
    .await?;
    Ok(vec![workspace])
}
