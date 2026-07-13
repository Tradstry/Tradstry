-- Offline-first journal sync: a client-minted HLC stamp for whole-row LWW and a
-- soft-delete tombstone (journal entries have downstream trade_tags /
-- trade_principle_violations junctions, and a client that never sees a
-- tombstone can't tell "deleted" from "not yet pushed").
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE journal_entries ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT now();

CREATE OR REPLACE TRIGGER trg_journal_entries_updated_at
    BEFORE UPDATE ON journal_entries FOR EACH ROW EXECUTE FUNCTION set_updated_at();

-- Account-scoped pull cursor scan: journal entries are account-scoped (unlike
-- playbooks), so both user_id and account_id gate the cursor comparison.
CREATE INDEX IF NOT EXISTS idx_journal_entries_cursor ON journal_entries (user_id, account_id, updated_at);
