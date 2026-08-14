CREATE TABLE IF NOT EXISTS brokerage_reconciliation_state (
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL,
    snaptrade_account_id TEXT NOT NULL,
    diagnostic_id TEXT NOT NULL,
    transaction_status TEXT NOT NULL DEFAULT 'not_checked',
    transaction_checked_at TIMESTAMPTZ,
    broker_transaction_count INTEGER NOT NULL DEFAULT 0,
    mapped_transaction_count INTEGER NOT NULL DEFAULT 0,
    imported_transaction_count INTEGER NOT NULL DEFAULT 0,
    duplicate_transaction_count INTEGER NOT NULL DEFAULT 0,
    skipped_transaction_count INTEGER NOT NULL DEFAULT 0,
    pending_transaction_count INTEGER NOT NULL DEFAULT 0,
    failed_transaction_count INTEGER NOT NULL DEFAULT 0,
    local_transaction_count INTEGER NOT NULL DEFAULT 0,
    missing_transaction_count INTEGER NOT NULL DEFAULT 0,
    extra_transaction_count INTEGER NOT NULL DEFAULT 0,
    portfolio_status TEXT NOT NULL DEFAULT 'not_checked',
    portfolio_checked_at TIMESTAMPTZ,
    broker_holding_count INTEGER NOT NULL DEFAULT 0,
    mapped_holding_count INTEGER NOT NULL DEFAULT 0,
    local_holding_count INTEGER NOT NULL DEFAULT 0,
    broker_balance_count INTEGER NOT NULL DEFAULT 0,
    local_balance_count INTEGER NOT NULL DEFAULT 0,
    balance_discrepancy_count INTEGER NOT NULL DEFAULT 0,
    transaction_error TEXT,
    portfolio_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, workspace_id, snaptrade_account_id),
    CONSTRAINT brokerage_reconciliation_workspace_owner_fk
        FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspaces(id, user_id) ON DELETE CASCADE,
    CONSTRAINT brokerage_reconciliation_transaction_status_check
        CHECK (transaction_status IN ('not_checked', 'pending', 'matched', 'discrepancy', 'failed')),
    CONSTRAINT brokerage_reconciliation_portfolio_status_check
        CHECK (portfolio_status IN ('not_checked', 'pending', 'matched', 'discrepancy', 'failed', 'unavailable')),
    CONSTRAINT brokerage_reconciliation_nonnegative_counts_check
        CHECK (
            broker_transaction_count >= 0 AND mapped_transaction_count >= 0 AND
            imported_transaction_count >= 0 AND duplicate_transaction_count >= 0 AND
            skipped_transaction_count >= 0 AND pending_transaction_count >= 0 AND
            failed_transaction_count >= 0 AND local_transaction_count >= 0 AND
            missing_transaction_count >= 0 AND extra_transaction_count >= 0 AND
            broker_holding_count >= 0 AND mapped_holding_count >= 0 AND
            local_holding_count >= 0 AND broker_balance_count >= 0 AND
            local_balance_count >= 0 AND balance_discrepancy_count >= 0
        )
);

CREATE INDEX IF NOT EXISTS idx_brokerage_reconciliation_workspace
    ON brokerage_reconciliation_state (user_id, workspace_id, updated_at DESC);

CREATE OR REPLACE TRIGGER trg_brokerage_reconciliation_updated_at
    BEFORE UPDATE ON brokerage_reconciliation_state
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
