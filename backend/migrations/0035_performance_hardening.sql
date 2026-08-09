-- Query-aligned indexes for the GraphQL hot paths. This migration is
-- transactional so a failed deploy cannot leave partially-created indexes.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS idx_journal_entries_user_workspace_created_live
    ON journal_entries (user_id, workspace_id, created_at DESC, id DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_journal_entries_user_workspace_close_live
    ON journal_entries (user_id, workspace_id, close_date DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_brokerage_tx_user_workspace_date_id
    ON brokerage_transactions (user_id, workspace_id, trade_date DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_brokerage_tx_workspace_type_date
    ON brokerage_transactions (user_id, workspace_id, transaction_type, trade_date DESC);

CREATE INDEX IF NOT EXISTS idx_brokerage_tx_symbol_search_trgm
    ON brokerage_transactions USING GIN ((
        COALESCE(symbol, '') || ' ' ||
        COALESCE(underlying_symbol, '') || ' ' ||
        COALESCE(symbol_description, '') || ' ' ||
        COALESCE(raw_symbol, '')
    ) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS idx_notebook_notes_user_workspace_order_live
    ON notebook_notes (user_id, workspace_id, sort_order ASC, updated_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_playbooks_user_workspace_created_live
    ON playbooks (user_id, workspace_id, created_at DESC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_principles_user_workspace_priority_live
    ON trading_principles (user_id, workspace_id, priority DESC, created_at ASC)
    WHERE deleted_at IS NULL;

CREATE INDEX IF NOT EXISTS idx_position_history_user_workspace_created
    ON position_calculator_history (user_id, workspace_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_position_plans_user_workspace_created
    ON position_calculator_plans (user_id, workspace_id, created_at DESC);

-- These secondary indexes duplicate an existing UNIQUE constraint or primary
-- key and only add write amplification.
DROP INDEX IF EXISTS idx_users_clerk_uuid;
DROP INDEX IF EXISTS idx_aeh_workspace_date;
DROP INDEX IF EXISTS idx_aeh_account_date;
