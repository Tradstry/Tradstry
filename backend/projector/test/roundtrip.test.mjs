import { test } from "node:test";
import assert from "node:assert/strict";
import { createHeadlessEditor } from "@lexical/headless";
import { HeadingNode, QuoteNode } from "@lexical/rich-text";
import { ListItemNode, ListNode } from "@lexical/list";
import { CodeHighlightNode, CodeNode } from "@lexical/code";
import { AutoLinkNode, LinkNode } from "@lexical/link";
import { HorizontalRuleNode } from "@lexical/react/LexicalHorizontalRuleNode";
import { jsonToUpdate } from "../seed.mjs";
import { updateToJson } from "../project.mjs";

// Independent node registry (not nodes.mjs) so the canonical form is stable even
// when a drift test deletes a node from nodes.mjs — only the projector breaks.
import {
  DEFAULT_NOTE_DOC,
  GhostTextNode,
  LinkedTradeNode,
  NotebookImageNode,
  NotebookVideoNode,
} from "../notebook-core.gen.mjs";

const REFERENCE_NODES = [
  NotebookImageNode,
  NotebookVideoNode,
  LinkedTradeNode,
  GhostTextNode,
  HeadingNode,
  QuoteNode,
  ListNode,
  ListItemNode,
  CodeNode,
  CodeHighlightNode,
  AutoLinkNode,
  LinkNode,
  HorizontalRuleNode,
];

const text = (t, format = 0) => ({
  type: "text",
  text: t,
  format,
  detail: 0,
  mode: "normal",
  style: "",
  version: 1,
});

const block = (extra) => ({
  direction: null,
  format: "",
  indent: 0,
  version: 1,
  ...extra,
});

const wrap = (children) => ({
  root: {
    children,
    direction: null,
    format: "",
    indent: 0,
    type: "root",
    version: 1,
  },
});

// Format bitmask (lexical): bold=1 italic=2 code=16 subscript=32 superscript=64.
const CASES = {
  paragraph: [block({ type: "paragraph", children: [text("hi")] })],

  headings: [
    block({ type: "heading", tag: "h1", children: [text("H1")] }),
    block({ type: "heading", tag: "h2", children: [text("H2")] }),
    block({ type: "heading", tag: "h3", children: [text("H3")] }),
  ],

  quote: [block({ type: "quote", children: [text("quoted")] })],

  bulletedList: [
    block({
      type: "list",
      listType: "bullet",
      tag: "ul",
      start: 1,
      children: [
        block({ type: "listitem", value: 1, children: [text("one")] }),
        block({ type: "listitem", value: 2, children: [text("two")] }),
      ],
    }),
  ],

  numberedList: [
    block({
      type: "list",
      listType: "number",
      tag: "ol",
      start: 1,
      children: [
        block({ type: "listitem", value: 1, children: [text("first")] }),
        block({ type: "listitem", value: 2, children: [text("second")] }),
      ],
    }),
  ],

  checkList: [
    block({
      type: "list",
      listType: "check",
      tag: "ul",
      start: 1,
      children: [
        block({
          type: "listitem",
          value: 1,
          checked: true,
          children: [text("done")],
        }),
        block({
          type: "listitem",
          value: 2,
          checked: false,
          children: [text("todo")],
        }),
      ],
    }),
  ],

  codeBlock: [
    block({
      type: "code",
      language: "rust",
      children: [text("fn main() {}")],
    }),
  ],

  link: [
    block({
      type: "paragraph",
      children: [
        block({
          type: "link",
          url: "https://example.com",
          rel: null,
          target: null,
          title: null,
          children: [text("site")],
        }),
      ],
    }),
  ],

  autolink: [
    block({
      type: "paragraph",
      children: [
        block({
          type: "autolink",
          url: "https://auto.example.com",
          rel: null,
          target: null,
          title: null,
          isUnlinked: false,
          children: [text("auto")],
        }),
      ],
    }),
  ],

  horizontalRule: [{ type: "horizontalrule", version: 1 }],

  formattedText: [
    block({
      type: "paragraph",
      children: [
        text("b", 1),
        text("i", 2),
        text("c", 16),
        text("sub", 32),
        text("sup", 64),
      ],
    }),
  ],

  nestedList: [
    block({
      type: "list",
      listType: "bullet",
      tag: "ul",
      start: 1,
      children: [
        block({
          type: "listitem",
          value: 1,
          children: [
            text("outer"),
            block({
              type: "list",
              listType: "bullet",
              tag: "ul",
              start: 1,
              children: [
                block({
                  type: "listitem",
                  value: 1,
                  children: [text("inner")],
                }),
              ],
            }),
          ],
        }),
      ],
    }),
  ],
  notebookImage: [
    block({
      type: "notebook-image",
      imageId: "img-1",
      src: "s3://bucket/chart.png",
      altText: "entry chart",
      width: 640,
      height: 480,
    }),
  ],
  notebookVideo: [
    block({
      type: "notebook-video",
      videoId: "vid-1",
      src: "s3://bucket/replay.mp4",
      altText: "replay",
    }),
  ],
  linkedTrade: [block({ type: "linked-trade", tradeId: "trade-42" })],

};

// Canonical serialization Lexical itself produces for a document — parse then
// re-serialize. This is what a faithful CRDT round trip must reproduce.
function canonical(doc) {
  const editor = createHeadlessEditor({
    namespace: "tradstry-notebook",
    nodes: REFERENCE_NODES,
    onError: (e) => {
      throw e;
    },
  });
  editor.setEditorState(editor.parseEditorState(JSON.stringify(doc)));
  // toJSON() keeps undefined-valued keys (e.g. `checked`, `theme`); JSON.stringify
  // drops them. updateToJson already serializes, so send canonical through the same
  // boundary rather than weaken the equality check.
  return JSON.parse(JSON.stringify(editor.getEditorState().toJSON()));
}

for (const [name, children] of Object.entries(CASES)) {
  test(`round-trips ${name}`, () => {
    const doc = wrap(children);
    const update = jsonToUpdate(JSON.stringify(doc));
    const out = JSON.parse(updateToJson([update]));
    assert.deepEqual(out, canonical(doc));
  });
}

// Ghost text is a transient AI suggestion. It must PARSE (the node is registered,
// so a CRDT carrying it does not break the projection) but must never reach
// `document_json`.
test("ghost text parses but is stripped from the projection", () => {
  const doc = wrap([
    block({
      type: "paragraph",
      children: [text("real")],
      textFormat: 0,
      textStyle: "",
    }),
    block({ type: "ghost-text", text: "AI suggestion" }),
  ]);

  const update = jsonToUpdate(JSON.stringify(doc));
  const out = JSON.parse(updateToJson([update]));

  const types = out.root.children.map((c) => c.type);
  assert.ok(!types.includes("ghost-text"), `ghost-text leaked into the projection: ${types}`);
  assert.ok(types.includes("paragraph"), "stripping removed real content");
});

// The round-trip cases build their reference with the SAME node classes, so a
// structural bug shared by both sides (e.g. a wrong `isInline`) is invisible to
// them. These assert absolute structure against the web editor's declarations:
// image/video are block-level, linked-trade is inline.
test("block decorators stay top-level and are not wrapped in a paragraph", () => {
  const doc = wrap([
    block({
      type: "notebook-image",
      imageId: "i1",
      src: "s3://c.png",
      altText: "chart",
      width: 640,
      height: 480,
    }),
  ]);

  const out = JSON.parse(updateToJson([jsonToUpdate(JSON.stringify(doc))]));
  const top = out.root.children.map((c) => c.type);
  assert.deepEqual(top, ["notebook-image"], `image was wrapped: ${JSON.stringify(top)}`);
});

test("inline decorators live inside a paragraph", () => {
  const doc = wrap([
    block({
      type: "paragraph",
      textFormat: 0,
      textStyle: "",
      children: [{ type: "linked-trade", version: 1, tradeId: "t1" }],
    }),
  ]);

  const out = JSON.parse(updateToJson([jsonToUpdate(JSON.stringify(doc))]));
  assert.equal(out.root.children[0].type, "paragraph");
  assert.equal(out.root.children[0].children[0].type, "linked-trade");
});

test("the default note document seeds and projects to an h1 and a paragraph", () => {
  // Both clients create a note from this exact document, so a note minted on the
  // desktop and one minted on the web are structurally identical.
  const projected = JSON.parse(updateToJson([jsonToUpdate(DEFAULT_NOTE_DOC)]));
  const kinds = projected.root.children.map((c) => c.type);
  assert.deepEqual(kinds, ["heading", "paragraph"]);
  assert.equal(projected.root.children[0].tag, "h1");
});
