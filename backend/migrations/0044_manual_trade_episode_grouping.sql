-- Broker executions stay immutable, while users may correct which executions
-- make up a journal trade. Manual episodes survive deterministic rebuilds;
-- their claimed transactions are removed from the automatic grouping input.
ALTER TABLE trade_episodes
    ADD COLUMN IF NOT EXISTS grouping_source TEXT NOT NULL DEFAULT 'automatic'
        CHECK (grouping_source IN ('automatic', 'manual'));

CREATE INDEX IF NOT EXISTS idx_trade_episodes_manual_grouping
    ON trade_episodes (user_id, workspace_id, grouping_source);
