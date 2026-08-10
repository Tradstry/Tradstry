-- SnapTrade's adapter contract is freshness-aware and webhook-driven. Persist
-- both pieces so delayed accounts are only pulled after SnapTrade advances and
-- webhook delivery is durable/idempotent before the endpoint acknowledges it.

ALTER TABLE brokerage_connections
    ADD COLUMN IF NOT EXISTS data_freshness_mode TEXT NOT NULL DEFAULT 'unknown';

ALTER TABLE brokerage_connections
    ADD CONSTRAINT brokerage_connections_freshness_mode_check
    CHECK (data_freshness_mode IN ('unknown', 'realtime', 'delayed'));

ALTER TABLE brokerage_sync_state
    ADD COLUMN IF NOT EXISTS holdings_last_successful_sync TEXT;

CREATE TABLE IF NOT EXISTS snaptrade_webhook_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    snaptrade_user_id TEXT NOT NULL,
    snaptrade_account_id TEXT,
    snaptrade_connection_id TEXT,
    event_timestamp TIMESTAMPTZ NOT NULL,
    details JSONB,
    normalized_event JSONB NOT NULL,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS snaptrade_webhook_events_claimable_idx
    ON snaptrade_webhook_events (next_attempt_at, event_timestamp)
    WHERE processed_at IS NULL AND attempts < 8;
