ALTER TABLE brokerage_connections
    ADD COLUMN IF NOT EXISTS last_sync_id TEXT,
    ADD COLUMN IF NOT EXISTS last_sync_succeeded_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_sync_transactions_synced INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_sync_holdings_synced INTEGER NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS last_sync_balances_synced INTEGER NOT NULL DEFAULT 0;

