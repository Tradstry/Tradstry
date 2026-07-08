// In-memory notebook model. Field names mirror the backend GraphQL shape
// (documentJson is Lexical editor-state JSON, exactly like the web) so wiring
// the real API later is a drop-in swap.

export type Folder = {
  id: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
};

export type Note = {
  id: string;
  folderId: string | null;
  title: string;
  /** Lexical `SerializedEditorState` JSON string (same format as the web). */
  documentJson: string;
  tradeIds: string[];
  sortOrder: number;
  updatedAt: string;
};

// --- Minimal Lexical editor-state builders (seed content only) -------------

type LexNode = Record<string, unknown>;

/** Text format bitmask: bold=1, italic=2, strikethrough=4, underline=8, code=16. */
export function text(t: string, format = 0): LexNode {
  return {
    detail: 0,
    format,
    mode: "normal",
    style: "",
    text: t,
    type: "text",
    version: 1,
  };
}
export function paragraph(...children: LexNode[]): LexNode {
  return {
    children: children.length ? children : [],
    direction: "ltr",
    format: "",
    indent: 0,
    type: "paragraph",
    version: 1,
  };
}
export function heading(tag: "h1" | "h2" | "h3", t: string): LexNode {
  return {
    children: [text(t)],
    direction: "ltr",
    format: "",
    indent: 0,
    type: "heading",
    tag,
    version: 1,
  };
}
export function quote(t: string): LexNode {
  return {
    children: [text(t)],
    direction: "ltr",
    format: "",
    indent: 0,
    type: "quote",
    version: 1,
  };
}
export function docJson(...children: LexNode[]): string {
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

export const EMPTY_DOC = docJson(paragraph());

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
        if (parts.join(" ").length > 140) return;
      }
    };
    walk(parsed.root?.children);
    return parts.join(" ").trim();
  } catch {
    return "";
  }
}

// --- Seed data -------------------------------------------------------------

export const SEED_FOLDERS: Folder[] = [
  { id: "f-journal", parentFolderId: null, name: "Daily journal", sortOrder: 0 },
  { id: "f-setups", parentFolderId: null, name: "Setups & playbooks", sortOrder: 1 },
  { id: "f-ideas", parentFolderId: null, name: "Ideas", sortOrder: 2 },
];

export const SEED_NOTES: Note[] = [
  {
    id: "n-1",
    folderId: "f-journal",
    title: "Jul 7 — choppy open, patient exit",
    documentJson: docJson(
      heading("h2", "Jul 7 — choppy open"),
      paragraph(
        text("Waited for the opening range to resolve instead of forcing it. Took "),
        text("NVDA long", 1),
        text(" on the reclaim of VWAP."),
      ),
      paragraph(
        text("Sized down because the tape was thin — good call, it whipped twice before the move."),
      ),
      quote("Discipline over conviction. The plan was to wait, and waiting paid."),
    ),
    tradeIds: [],
    sortOrder: 0,
    updatedAt: "2026-07-07T18:04:00Z",
  },
  {
    id: "n-2",
    folderId: "f-setups",
    title: "VWAP reclaim — checklist",
    documentJson: docJson(
      heading("h2", "VWAP reclaim"),
      paragraph(text("Entry criteria before I press the button:")),
      paragraph(
        text("Trend day context · reclaim on rising volume · stop under the reclaim candle · target = prior day high."),
      ),
    ),
    tradeIds: [],
    sortOrder: 0,
    updatedAt: "2026-07-06T14:20:00Z",
  },
  {
    id: "n-3",
    folderId: "f-ideas",
    title: "Watchlist — earnings drift",
    documentJson: docJson(
      heading("h2", "Earnings drift ideas"),
      paragraph(text("Names that gapped and held — worth stalking for continuation.")),
    ),
    tradeIds: [],
    sortOrder: 0,
    updatedAt: "2026-07-05T09:10:00Z",
  },
  {
    id: "n-4",
    folderId: "f-journal",
    title: "Weekly review — first green week",
    documentJson: docJson(
      heading("h2", "Weekly review"),
      paragraph(
        text("Cut size on B setups and it immediately smoothed the equity curve. Keep doing that."),
      ),
    ),
    tradeIds: [],
    sortOrder: 1,
    updatedAt: "2026-07-04T20:00:00Z",
  },
];
