-- Star = a personal favourite flag (the note surfaces in a virtual Favourites view but stays
-- in its real folder); pin = float the note to the top of its list. Both are lightweight
-- per-note booleans defaulting off, so every existing note starts unstarred and unpinned.
ALTER TABLE notebook_notes ADD COLUMN IF NOT EXISTS is_starred BOOLEAN NOT NULL DEFAULT false;
ALTER TABLE notebook_notes ADD COLUMN IF NOT EXISTS is_pinned BOOLEAN NOT NULL DEFAULT false;

-- Partial indexes: the Favourites view and the pinned-first sort both filter on the true side,
-- which is the small minority of rows, so a partial index stays tiny and skips the rest.
CREATE INDEX IF NOT EXISTS idx_notebook_notes_starred
    ON notebook_notes (user_id, account_id) WHERE is_starred;
CREATE INDEX IF NOT EXISTS idx_notebook_notes_pinned
    ON notebook_notes (user_id, account_id) WHERE is_pinned;
