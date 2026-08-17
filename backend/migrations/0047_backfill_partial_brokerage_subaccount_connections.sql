-- Some imported subaccount workspaces were left with shared SnapTrade
-- credentials but without the brokerage authorization/account binding. If a
-- workspace has exactly one historical SnapTrade account id in sync state, bind
-- it to the same authorization that owns the shared credentials.

WITH historical_bindings AS (
    SELECT
        user_id,
        workspace_id,
        MIN(snaptrade_account_id) AS snaptrade_account_id
    FROM brokerage_sync_state
    WHERE snaptrade_account_id IS NOT NULL
      AND snaptrade_account_id <> ''
    GROUP BY user_id, workspace_id
    HAVING COUNT(DISTINCT snaptrade_account_id) = 1
),
source_connections AS (
    SELECT DISTINCT ON (
        user_id,
        snaptrade_user_id,
        snaptrade_user_secret_encrypted
    )
        user_id,
        snaptrade_user_id,
        snaptrade_user_secret_encrypted,
        snaptrade_connection_id,
        broker,
        data_freshness_mode
    FROM brokerage_connections
    WHERE snaptrade_user_id IS NOT NULL
      AND snaptrade_user_secret_encrypted IS NOT NULL
      AND snaptrade_connection_id IS NOT NULL
    ORDER BY
        user_id,
        snaptrade_user_id,
        snaptrade_user_secret_encrypted,
        updated_at DESC
),
repairs AS (
    SELECT
        target.user_id,
        target.workspace_id,
        historical_bindings.snaptrade_account_id,
        source_connections.snaptrade_connection_id,
        source_connections.broker,
        source_connections.data_freshness_mode
    FROM brokerage_connections target
    JOIN historical_bindings
      ON historical_bindings.user_id = target.user_id
     AND historical_bindings.workspace_id = target.workspace_id
    JOIN source_connections
      ON source_connections.user_id = target.user_id
     AND source_connections.snaptrade_user_id = target.snaptrade_user_id
     AND source_connections.snaptrade_user_secret_encrypted = target.snaptrade_user_secret_encrypted
    WHERE target.snaptrade_user_id IS NOT NULL
      AND target.snaptrade_user_secret_encrypted IS NOT NULL
      AND target.snaptrade_connection_id IS NULL
      AND target.snaptrade_account_id IS NULL
      AND NOT EXISTS (
          SELECT 1
          FROM brokerage_connections used
          WHERE used.user_id = target.user_id
            AND used.snaptrade_account_id = historical_bindings.snaptrade_account_id
      )
)
UPDATE brokerage_connections target
SET snaptrade_connection_id = repairs.snaptrade_connection_id,
    snaptrade_account_id = repairs.snaptrade_account_id,
    broker = COALESCE(NULLIF(target.broker, ''), repairs.broker),
    data_freshness_mode = repairs.data_freshness_mode,
    updated_at = now()
FROM repairs
WHERE target.user_id = repairs.user_id
  AND target.workspace_id = repairs.workspace_id;
