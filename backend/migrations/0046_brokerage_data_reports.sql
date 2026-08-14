-- User-reported brokerage discrepancies. The snapshot is assembled from
-- Tradstry-owned status and count fields; raw provider payloads and credentials
-- are never accepted from the client or stored here.
CREATE TABLE IF NOT EXISTS brokerage_data_reports (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    workspace_id TEXT NOT NULL,
    snaptrade_account_id TEXT NOT NULL,
    diagnostic_id TEXT NOT NULL,
    category TEXT NOT NULL
        CHECK (category IN ('transactions', 'holdings', 'balances', 'account', 'other')),
    note TEXT,
    diagnostic_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    status TEXT NOT NULL DEFAULT 'open'
        CHECK (status IN ('open', 'resolved', 'dismissed')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT brokerage_data_reports_workspace_owner_fk
        FOREIGN KEY (workspace_id, user_id)
        REFERENCES workspaces(id, user_id) ON DELETE CASCADE,
    CONSTRAINT brokerage_data_reports_note_length_check
        CHECK (note IS NULL OR char_length(note) <= 1000)
);

CREATE INDEX IF NOT EXISTS idx_brokerage_data_reports_review_queue
    ON brokerage_data_reports (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_brokerage_data_reports_workspace
    ON brokerage_data_reports (user_id, workspace_id, created_at DESC);

CREATE OR REPLACE TRIGGER trg_brokerage_data_reports_updated_at
    BEFORE UPDATE ON brokerage_data_reports
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();
