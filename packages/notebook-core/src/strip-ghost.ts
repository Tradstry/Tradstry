type LexicalNodeJson = { type?: string; children?: LexicalNodeJson[] };
type EditorStateJson = { root?: { children?: LexicalNodeJson[] } };

const GHOST_TYPE = "ghost-text";

/**
 * Ghost text is a transient AI suggestion. It must parse (so a CRDT carrying it
 * does not break the projection) but must never reach the stored `document_json`.
 */
export function stripGhostText<T extends EditorStateJson>(state: T): T {
  const strip = (nodes: LexicalNodeJson[] = []): LexicalNodeJson[] =>
    nodes
      .filter((n) => n?.type !== GHOST_TYPE)
      .map((n) => (n?.children ? { ...n, children: strip(n.children) } : n));

  if (!state?.root?.children) return state;
  return { ...state, root: { ...state.root, children: strip(state.root.children) } };
}
