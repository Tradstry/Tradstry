type LexNode = Record<string, unknown>;

const PREVIEW_MAX_CHARS = 140;

function paragraph(...children: LexNode[]): LexNode {
  return {
    children: children.length ? children : [],
    direction: "ltr",
    format: "",
    indent: 0,
    type: "paragraph",
    version: 1,
  };
}

function docJson(...children: LexNode[]): string {
  return JSON.stringify({
    root: {
      children,
      direction: "ltr",
      format: "",
      indent: 0,
      type: "root",
      version: 1,
    },
  });
}

function heading(tag: string): LexNode {
  return {
    children: [],
    direction: "ltr",
    format: "",
    indent: 0,
    type: "heading",
    version: 1,
    tag,
  };
}

/**
 * The document every new note is seeded from, on every client. The creating device
 * seeds the note's Y.Doc from this, so a note minted on the desktop and one minted
 * on the web must start from the same structure or the two clients diverge.
 */
export const DEFAULT_NOTE_DOC = docJson(heading("h1"), paragraph());

/** Best-effort plain-text preview from a note's documentJson (for the list). */
export function notePreview(documentJson: string): string {
  try {
    const parsed = JSON.parse(documentJson) as {
      root?: { children?: LexNode[] };
    };
    const parts: string[] = [];
    const walk = (nodes: LexNode[] | undefined) => {
      for (const n of nodes ?? []) {
        if (n.type === "text" && typeof n.text === "string") parts.push(n.text);
        if (Array.isArray(n.children)) walk(n.children as LexNode[]);
        if (parts.join(" ").length > PREVIEW_MAX_CHARS) return;
      }
    };
    walk(parsed.root?.children);
    return parts.join(" ").trim();
  } catch {
    return "";
  }
}
