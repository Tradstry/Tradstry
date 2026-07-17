import assert from "node:assert/strict";
import { test } from "node:test";
import {
  appendToJson,
  applyMarkdown,
  markdownToJson,
} from "../markdown.mjs";
import { updateToJson } from "../project.mjs";
import { jsonToUpdate } from "../seed.mjs";

/** Flatten a document_json's text so assertions read against content, not structure. */
function textOf(documentJson) {
  const out = [];
  const walk = (node) => {
    if (typeof node.text === "string") out.push(node.text);
    for (const child of node.children ?? []) walk(child);
  };
  walk(JSON.parse(documentJson).root);
  return out.join("\n");
}

function headings(documentJson) {
  const out = [];
  const walk = (node) => {
    if (node.type === "heading") {
      const text = (node.children ?? []).map((c) => c.text ?? "").join("");
      out.push(`${node.tag}:${text}`);
    }
    for (const child of node.children ?? []) walk(child);
  };
  walk(JSON.parse(documentJson).root);
  return out;
}

test("markdown becomes real Lexical structure, not a wall of text", () => {
  const json = markdownToJson(
    "# Weekly Report\n\nYou traded **well**.\n\n## Mistakes\n\n- chased AEHR\n- chased INOD\n",
  );
  assert.deepEqual(headings(json), ["h1:Weekly Report", "h2:Mistakes"]);
  const text = textOf(json);
  assert.match(text, /chased AEHR/);
  assert.match(text, /chased INOD/);
});

test("append adds to the document without destroying what is there", () => {
  const seed = jsonToUpdate(markdownToJson("# Log\n\nWeek 1 notes.\n"));
  const delta = applyMarkdown([seed], "## Week 2\n\nWeek 2 notes.\n", "append");

  const text = textOf(updateToJson([seed, delta]));
  assert.match(text, /Week 1 notes/, "existing content survived");
  assert.match(text, /Week 2 notes/, "new content landed");
  assert.equal(
    headings(updateToJson([seed, delta])).join(","),
    "h1:Log,h2:Week 2",
  );
});

test("replace swaps the body", () => {
  const seed = jsonToUpdate(markdownToJson("# Old\n\nStale content.\n"));
  const delta = applyMarkdown([seed], "# New\n\nFresh content.\n", "replace");

  const text = textOf(updateToJson([seed, delta]));
  assert.match(text, /Fresh content/);
  assert.doesNotMatch(text, /Stale content/);
});

/**
 * The bug this guards: rebuilding a Y.Doc from scratch instead of emitting a delta does
 * not conflict with the live document, it concatenates — every paragraph silently doubles.
 * Applying the delta to the ORIGINAL history must yield the content exactly once.
 */
test("an edit is a delta against history, so nothing is duplicated", () => {
  const seed = jsonToUpdate(markdownToJson("# Report\n\nOriginal line.\n"));
  const delta = applyMarkdown([seed], "Added line.\n", "append");

  const text = textOf(updateToJson([seed, delta]));
  assert.equal(text.match(/Original line\./g).length, 1);
  assert.equal(text.match(/Added line\./g).length, 1);
});

test("editing a note with no history is refused rather than silently reseeded", () => {
  assert.throws(() => applyMarkdown([], "# Hi\n", "replace"), /no updates/);
});

test("appending to a legacy note keeps nodes markdown cannot express", () => {
  const base = JSON.parse(markdownToJson("# Review\n\nSome text.\n"));
  // A notebook image: markdown has no way to represent it, so a markdown round-trip
  // would drop it. Structural append must not.
  base.root.children.push({
    type: "notebook-image",
    version: 1,
    hash: "abc123",
    altText: "chart",
    width: 800,
    height: 600,
  });

  const out = appendToJson(JSON.stringify(base), "## Addendum\n\nMore text.\n");
  const parsed = JSON.parse(out);
  const types = parsed.root.children.map((c) => c.type);

  assert.ok(types.includes("notebook-image"), "image node survived the append");
  assert.match(textOf(out), /Some text/);
  assert.match(textOf(out), /More text/);
});
