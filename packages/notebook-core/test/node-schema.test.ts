import { expect, test } from "bun:test";
import { createHeadlessEditor } from "@lexical/headless";
import { NAMESPACE } from "../src/contract";
import { NODES } from "../src/nodes";
import {
  NotebookImageNode,
  NotebookVideoNode,
  type SerializedNotebookImageNode,
  type SerializedNotebookVideoNode,
} from "../src/nodes/custom";

function inEditor<T>(fn: () => T): T {
  const editor = createHeadlessEditor({
    namespace: NAMESPACE,
    nodes: NODES,
    onError: (e) => {
      throw e;
    },
  });
  let out!: T;
  editor.update(
    () => {
      out = fn();
    },
    { discrete: true },
  );
  return out;
}

test("image node round-trips hash-only wire format", () => {
  const json = inEditor(() => {
    const node = NotebookImageNode.importJSON({
      type: "notebook-image",
      version: 2,
      hash: "abc123",
      altText: "chart",
      width: 800,
      height: 600,
    });
    return node.exportJSON() as SerializedNotebookImageNode & { src?: string };
  });
  expect(json.version).toBe(2);
  expect(json.hash).toBe("abc123");
  expect("src" in json).toBe(false);
  expect(json.width).toBe(800);
});

test("video node round-trips hash-only wire format", () => {
  const json = inEditor(() => {
    const node = NotebookVideoNode.importJSON({
      type: "notebook-video",
      version: 2,
      hash: "def456",
      altText: "clip",
    });
    return node.exportJSON() as SerializedNotebookVideoNode & { src?: string };
  });
  expect(json.hash).toBe("def456");
  expect("src" in json).toBe(false);
});
