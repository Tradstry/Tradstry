-- Content-addressed notebook media. A note's image/video Lexical nodes reference
-- the SHA-256 of the bytes; the same bytes can be referenced from many notes, so
-- the old global-unique columns no longer hold. Dedup identity is (user, hash);
-- one row per (user, note, hash).
ALTER TABLE notebook_images ADD COLUMN IF NOT EXISTS content_hash TEXT NOT NULL DEFAULT '';

ALTER TABLE notebook_images DROP CONSTRAINT IF EXISTS notebook_images_cloudinary_asset_id_key;
ALTER TABLE notebook_images DROP CONSTRAINT IF EXISTS notebook_images_cloudinary_public_id_key;

-- Partial: pre-migration rows all backfill to '', and a note may legitimately hold
-- several of them, so uniqueness can only be enforced once a row actually has a hash.
DROP INDEX IF EXISTS ux_notebook_images_note_hash;
CREATE UNIQUE INDEX ux_notebook_images_note_hash
    ON notebook_images (user_id, note_id, content_hash)
    WHERE content_hash <> '';
CREATE INDEX IF NOT EXISTS idx_notebook_images_user_hash
    ON notebook_images (user_id, content_hash);
