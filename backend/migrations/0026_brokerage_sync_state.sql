-- Remember how far SnapTrade has synced each brokerage account's transactions,
-- so we only re-fetch when there is something new.
--
-- SnapTrade caches transactions and refreshes them once per day, a day behind
-- real time, and exposes the position via sync_status.transactions.
-- last_successful_sync ("all transactions up to this date have been successfully
-- synced") on the list-accounts response we already call on every sync. Polling
-- activities every 30 minutes therefore asked ~14 times a day for data that can
-- move at most once; their guidance is at most once per account per 24h.
--
-- Keyed by snaptrade_account_id rather than our account_id: one Tradstry account
-- can map to several SnapTrade accounts (e.g. a Webull margin and cash account),
-- each syncing on its own schedule.
--
-- SnapTrade regenerates these ids when a user is re-registered, so a reconnect
-- leaves no matching row, the gate misses, and a full re-sync happens on its
-- own — recovery needs no special case.

CREATE TABLE IF NOT EXISTS brokerage_sync_state (
    user_id                           TEXT        NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id                        TEXT        NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    snaptrade_account_id              TEXT        NOT NULL,
    -- Day-granular date string exactly as SnapTrade returns it (YYYY-MM-DD).
    transactions_last_successful_sync TEXT,
    updated_at                        TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, account_id, snaptrade_account_id)
);

CREATE INDEX IF NOT EXISTS idx_brokerage_sync_state_account
    ON brokerage_sync_state (user_id, account_id);
