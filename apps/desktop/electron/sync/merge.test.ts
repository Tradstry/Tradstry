import assert from "node:assert/strict";
import test from "node:test";
import { mergeNote, type NoteRow } from "./merge.ts";

function note(id = "n1"): NoteRow {
  return {
    id,
    folderId: null,
    title: "Untitled",
    documentJson: "A",
    sortOrder: 0,
    tradeIds: [],
    hlcFolderId: "",
    hlcSortOrder: "",
    hlcTradeIds: "",
    bodyHlc: "",
    deletedAt: null,
  };
}

test("server tombstone beats a local edit", () => {
  assert.equal(mergeNote(note(), { ...note(), deletedAt: "2026-01-01" }).kind, "tombstone");
});

test("per-field LWW keeps changes from both sides", () => {
  const local = { ...note(), folderId: "f1", hlcFolderId: "000000000000009:00000:c1" };
  const server = { ...note(), sortOrder: 7, hlcSortOrder: "000000000000009:00000:c2" };
  const result = mergeNote(local, server);
  assert.equal(result.kind, "take");
  if (result.kind === "take") {
    assert.equal(result.note.folderId, "f1");
    assert.equal(result.note.sortOrder, 7);
  }
});

test("trade ids compare as a set", () => {
  assert.equal(mergeNote({ ...note(), tradeIds: ["b", "a"] }, { ...note(), tradeIds: ["a", "b"] }).kind, "unchanged");
});
