-- Offline-first notebook sync: tombstones, HLC stamps, client mutation log.
-- Additive only. Existing readers are unaffected until they add a deleted_at guard.

ALTER TABLE notebook_notes   ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE notebook_folders ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;

-- Sortable HLC string: "<015 millis>:<05 counter>:<client_id>". Empty for
-- server-authored rows predating sync; compares less than any real stamp.
ALTER TABLE notebook_notes   ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE notebook_folders ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

-- Live-row reads.
CREATE INDEX IF NOT EXISTS idx_notebook_notes_live
    ON notebook_notes (account_id, updated_at) WHERE deleted_at IS NULL;
CREATE INDEX IF NOT EXISTS idx_notebook_folders_live
    ON notebook_folders (account_id, updated_at) WHERE deleted_at IS NULL;

-- Delta pull must see tombstones too, so this index has no WHERE clause.
CREATE INDEX IF NOT EXISTS idx_notebook_notes_cursor
    ON notebook_notes (account_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_notebook_folders_cursor
    ON notebook_folders (account_id, updated_at);

CREATE TABLE IF NOT EXISTS notebook_client_mutations (
    client_id        TEXT PRIMARY KEY NOT NULL,
    user_id          TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    last_mutation_id BIGINT NOT NULL DEFAULT 0,
    updated_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_notebook_client_mutations_user
    ON notebook_client_mutations (user_id);
