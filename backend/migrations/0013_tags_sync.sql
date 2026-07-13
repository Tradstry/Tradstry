-- Offline-first tags/tag_categories sync: a client-minted HLC stamp for whole-row
-- LWW and a soft-delete tombstone (tags feed the trade_tags junction, and a
-- client that never sees a tombstone can't tell "deleted" from "not yet pushed").
ALTER TABLE tag_categories ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE tag_categories ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';
ALTER TABLE tags ADD COLUMN IF NOT EXISTS deleted_at TIMESTAMPTZ NULL;
ALTER TABLE tags ADD COLUMN IF NOT EXISTS hlc TEXT NOT NULL DEFAULT '';

-- User-scoped pull cursor scan.
CREATE INDEX IF NOT EXISTS idx_tag_categories_cursor ON tag_categories (user_id, updated_at);
CREATE INDEX IF NOT EXISTS idx_tags_cursor ON tags (user_id, updated_at);
