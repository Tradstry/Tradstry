ALTER TABLE brokerage_connections
    ADD COLUMN IF NOT EXISTS last_sync_status TEXT NOT NULL DEFAULT 'idle',
    ADD COLUMN IF NOT EXISTS last_sync_error TEXT,
    ADD COLUMN IF NOT EXISTS last_sync_started_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS last_sync_finished_at TIMESTAMPTZ;

ALTER TABLE brokerage_connections
    DROP CONSTRAINT IF EXISTS brokerage_connections_last_sync_status_check;

ALTER TABLE brokerage_connections
    ADD CONSTRAINT brokerage_connections_last_sync_status_check
    CHECK (last_sync_status IN ('idle', 'queued', 'completed', 'failed'));
