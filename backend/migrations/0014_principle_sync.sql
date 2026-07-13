-- Offline-first principle sync: a client-minted HLC stamp for whole-row LWW
-- and a soft-delete tombstone, mirroring 0012_journal_sync.sql. Principles are
-- account-scoped but carry no downstream junction tables that need a
-- tombstone to disambiguate (trade_principle_violations rows are owned by the
-- journal entry side), so this migration is simpler than the journal one.
ALTER TABLE trading_principles ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE trading_principles ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

-- Account-scoped pull cursor scan: principles are account-scoped, so both
-- user_id and account_id gate the cursor comparison (mirrors
-- idx_journal_entries_cursor).
CREATE INDEX IF NOT EXISTS idx_trading_principles_cursor ON trading_principles (user_id, account_id, updated_at);
