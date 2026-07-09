-- Trading principles: account-scoped behavioral rules, optionally scoped to a
-- playbook. playbook_id NULL means the principle governs every trade in the
-- account.
CREATE TABLE IF NOT EXISTS trading_principles (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
    playbook_id TEXT REFERENCES playbooks(id) ON DELETE RESTRICT,
    evidence_note_id TEXT REFERENCES notebook_notes(id) ON DELETE SET NULL,
    title TEXT NOT NULL,
    the_rule TEXT NOT NULL,
    why TEXT NOT NULL,
    intervention TEXT,
    priority BIGINT NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_trading_principles_account_active
    ON trading_principles (account_id, is_active, priority DESC);
CREATE INDEX IF NOT EXISTS idx_trading_principles_user_id
    ON trading_principles (user_id);
CREATE INDEX IF NOT EXISTS idx_trading_principles_playbook
    ON trading_principles (playbook_id) WHERE playbook_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_trading_principles_evidence_note
    ON trading_principles (evidence_note_id) WHERE evidence_note_id IS NOT NULL;
CREATE OR REPLACE TRIGGER trg_trading_principles_updated_at
    BEFORE UPDATE ON trading_principles
    FOR EACH ROW EXECUTE FUNCTION set_updated_at();

CREATE TABLE IF NOT EXISTS trade_principle_violations (
    journal_entry_id TEXT NOT NULL REFERENCES journal_entries(id) ON DELETE CASCADE,
    principle_id TEXT NOT NULL REFERENCES trading_principles(id) ON DELETE CASCADE,
    PRIMARY KEY (journal_entry_id, principle_id)
);
CREATE INDEX IF NOT EXISTS idx_tpv_principle
    ON trade_principle_violations (principle_id);
