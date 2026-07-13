-- Offline-first playbook sync: a client-minted HLC stamp for whole-row LWW and a
-- soft-delete tombstone (hard delete is blocked by trading_principles' ON DELETE
-- RESTRICT, and a client that never sees a tombstone can't tell "deleted" from
-- "not yet pushed").
ALTER TABLE playbooks ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE playbooks ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

-- User-scoped pull cursor scan.
CREATE INDEX IF NOT EXISTS idx_playbooks_cursor ON playbooks (user_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_playbooks_live
    ON playbooks (user_id, updated_at) WHERE deleted_at IS NULL;
