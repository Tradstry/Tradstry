-- Stamps the replay algorithm that produced a stored curve. When the math changes the
-- constant is bumped, the stamp no longer matches, and the curve is rebuilt on next read
-- instead of silently serving numbers the current code would never produce.
ALTER TABLE account_equity_rebuild ADD COLUMN IF NOT EXISTS replay_version INTEGER NOT NULL DEFAULT 0;
