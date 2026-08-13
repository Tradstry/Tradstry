use anyhow::{Context, Result};
use sqlx::PgPool;
use std::collections::HashSet;

use super::client::SnapTradeAccount;
use crate::service::db::schema::tables::workspaces_table::{self, CreateWorkspaceInput, Workspace};

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
        && let Some(candidate) = snaptrade_accounts.iter().find(|candidate| {
            candidate.id.as_deref() == Some(bound_id)
                && connection_id.is_none_or(|connection_id| {
                    candidate.brokerage_authorization.as_deref() == Some(connection_id)
                })
        })
    {
        let workspace = ensure_broker_label(pool, user_id, workspace, candidate).await?;
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
    let workspace = ensure_broker_label(pool, user_id, workspace, selected.unwrap()).await?;
    Ok(vec![workspace])
}

async fn ensure_broker_label(
    pool: &PgPool,
    user_id: &str,
    workspace: Workspace,
    account: &SnapTradeAccount,
) -> Result<Workspace> {
    if workspace.broker.is_some() {
        return Ok(workspace);
    }
    let Some(institution) = account
        .institution_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(workspace);
    };
    workspaces_table::set_broker(pool, &workspace.id, user_id, institution).await
}

pub fn brokerage_account_name(account: &SnapTradeAccount) -> String {
    account
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .or_else(|| {
            account
                .institution_name
                .as_deref()
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(|name| format!("{name} Account"))
        })
        .unwrap_or_else(|| "Brokerage Account".to_string())
}

fn unique_workspace_name(preferred: &str, existing_names: &mut HashSet<String>) -> String {
    let normalized = preferred.to_lowercase();
    if existing_names.insert(normalized) {
        return preferred.to_string();
    }

    for suffix in 2.. {
        let candidate = format!("{preferred} ({suffix})");
        if existing_names.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    unreachable!("workspace name suffix search is unbounded")
}

/// Explicitly creates one workspace for each selected, unlinked account that
/// belongs to the source workspace's brokerage authorization. Repeated calls
/// are idempotent because an upstream account can be bound only once per user.
pub async fn create_workspaces_for_connection_accounts(
    pool: &PgPool,
    user_id: &str,
    source_workspace_id: &str,
    snaptrade_accounts: &[SnapTradeAccount],
    requested_account_ids: &HashSet<String>,
) -> Result<Vec<Workspace>> {
    let source = workspaces_table::find_workspace(pool, source_workspace_id, user_id)
        .await?
        .context("Source workspace not found")?;
    let snaptrade_user_id = source
        .snaptrade_user_id
        .as_deref()
        .context("Source workspace is not registered with SnapTrade")?;
    let encrypted_secret = source
        .snaptrade_user_secret_encrypted
        .as_deref()
        .context("Source workspace has no SnapTrade secret")?;
    let connection_id = source
        .snaptrade_connection_id
        .as_deref()
        .context("Source workspace has no brokerage connection")?;

    let existing = workspaces_table::list_workspaces(pool, user_id).await?;
    let mut linked_account_ids: HashSet<String> = existing
        .iter()
        .filter_map(|workspace| workspace.snaptrade_account_id.clone())
        .collect();
    let mut existing_names: HashSet<String> = existing
        .iter()
        .map(|workspace| workspace.name.to_lowercase())
        .collect();
    let mut created = Vec::new();

    for account in snaptrade_accounts {
        let Some(account_id) = account.id.as_deref() else {
            continue;
        };
        if account.brokerage_authorization.as_deref() != Some(connection_id)
            || !requested_account_ids.contains(account_id)
            || linked_account_ids.contains(account_id)
        {
            continue;
        }

        let name = unique_workspace_name(&brokerage_account_name(account), &mut existing_names);
        let workspace = workspaces_table::create_workspace(
            pool,
            user_id,
            CreateWorkspaceInput {
                name,
                icon: source.icon.clone(),
                currency: source.currency.clone(),
                risk_profile: source.risk_profile.clone(),
                asset_class: source.asset_class.clone(),
                broker: source
                    .broker
                    .clone()
                    .or_else(|| account.institution_name.clone()),
            },
        )
        .await?;

        let binding = async {
            workspaces_table::update_snaptrade_credentials(
                pool,
                &workspace.id,
                user_id,
                snaptrade_user_id,
                encrypted_secret,
                Some(connection_id),
            )
            .await?;
            workspaces_table::set_snaptrade_account_id(pool, &workspace.id, user_id, account_id)
                .await
        }
        .await;

        match binding {
            Ok(bound) => {
                linked_account_ids.insert(account_id.to_string());
                created.push(bound);
            }
            Err(error) => {
                if let Err(cleanup_error) =
                    workspaces_table::delete_workspace(pool, &workspace.id, user_id).await
                {
                    log::error!(
                        "Failed to remove partially imported workspace {}: {cleanup_error}",
                        workspace.id
                    );
                }
                return Err(error).context("Failed to bind imported brokerage workspace");
            }
        }
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::{brokerage_account_name, unique_workspace_name};
    use crate::service::brokerage::client::SnapTradeAccount;
    use std::collections::HashSet;

    fn account(name: Option<&str>, institution_name: Option<&str>) -> SnapTradeAccount {
        SnapTradeAccount {
            id: Some("account".to_string()),
            brokerage_authorization: Some("connection".to_string()),
            name: name.map(str::to_string),
            number: None,
            institution_name: institution_name.map(str::to_string),
            sync_status: None,
        }
    }

    #[test]
    fn imported_name_prefers_account_then_institution() {
        assert_eq!(
            brokerage_account_name(&account(Some("Individual Margin"), Some("Webull"))),
            "Individual Margin"
        );
        assert_eq!(
            brokerage_account_name(&account(None, Some("Webull"))),
            "Webull Account"
        );
        assert_eq!(
            brokerage_account_name(&account(None, None)),
            "Brokerage Account"
        );
    }

    #[test]
    fn imported_names_are_unique_case_insensitively() {
        let mut names = HashSet::from(["cash account".to_string()]);
        assert_eq!(
            unique_workspace_name("Cash Account", &mut names),
            "Cash Account (2)"
        );
        assert_eq!(
            unique_workspace_name("Cash Account", &mut names),
            "Cash Account (3)"
        );
    }
}
