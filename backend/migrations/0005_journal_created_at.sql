-- Immutable insertion-time column for STABLE keyset pagination on
-- (created_at, id). close_date is user-editable, so it can't anchor a cursor
-- without drift. Backfill existing rows to close_date (best available proxy),
-- then swap the pagination index to (user_id, created_at DESC, id DESC).

ALTER TABLE journal_entries
    ADD COLUMN IF NOT EXISTS created_at TIMESTAMPTZ NOT NULL DEFAULT now();

UPDATE journal_entries SET created_at = close_date;

DROP INDEX IF EXISTS idx_journal_entries_user_open_close;

CREATE INDEX IF NOT EXISTS idx_journal_entries_user_created_id
    ON journal_entries (user_id, created_at DESC, id DESC);
