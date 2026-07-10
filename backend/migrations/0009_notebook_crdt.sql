-- Yjs CRDT storage for note bodies. Additive; no existing note is `crdt` yet.

-- A note with no row here is `legacy`: document_json is authoritative.
CREATE TABLE IF NOT EXISTS notebook_note_crdt (
    note_id        TEXT PRIMARY KEY REFERENCES notebook_notes(id) ON DELETE CASCADE,
    state          TEXT NOT NULL CHECK (state IN ('seeding','crdt')),
    state_vector   BYTEA NOT NULL,
    -- NOT NULL: only the server seeds. A client that finds no row must refuse to
    -- bootstrap -- two independent Y.Docs merge by concatenation, duplicating content.
    crdt_seeded_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    projected_at   TIMESTAMPTZ NULL,
    -- The update seq the projection was built from. Fresh iff
    -- projected_seq >= MAX(notebook_note_updates.seq) for this note.
    projected_seq  BIGINT NOT NULL DEFAULT 0
);

-- Append-only. Never UPDATE a row here; INSERT, or DELETE during compaction.
CREATE TABLE IF NOT EXISTS notebook_note_updates (
    note_id    TEXT NOT NULL REFERENCES notebook_notes(id) ON DELETE CASCADE,
    seq        BIGSERIAL,
    update     BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (note_id, seq)
);
CREATE INDEX IF NOT EXISTS idx_notebook_note_updates_note
    ON notebook_note_updates (note_id, seq);

-- Monotonic version of the body an embedding was built from. Guards against a
-- retried/slow job overwriting a newer vector with older content.
ALTER TABLE ai_source_documents ADD COLUMN IF NOT EXISTS body_version BIGINT NOT NULL DEFAULT 0;
