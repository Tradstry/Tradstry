"use client";

import {
  DndContext,
  type DragEndEvent,
  type DragMoveEvent,
  type DragStartEvent,
  PointerSensor,
  pointerWithin,
  useDraggable,
  useDroppable,
  useSensor,
  useSensors,
} from "@dnd-kit/core";
import {
  Add01Icon,
  ArrowDown01Icon,
  ArrowRight01Icon,
  Delete02Icon,
  Folder01Icon,
  FolderAddIcon,
  Note01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback, useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  type NotebookTreeNode,
  useDeleteNotebookFolder,
  useMoveNotebookNode,
  useRenameNotebookFolder,
} from "@/hooks/notebook";
import type { JournalEntry } from "@/lib/types/journal";
import { cn } from "@/lib/utils";

// 24px per depth level. A folder row has a chevron (24px) + icon (24px) before
// its name; a note row has only an icon (24px). A 24px step therefore lines up
// each nested note's icon/label directly under its parent folder's name — the
// Notion-style indent.
const INDENT_STEP = 24;
const INDENT_BASE = 4;

// Notion-style dark tooltip. Overrides the default `bg-foreground` tooltip with
// the exact charcoal from the reference, and recolors the tooltip arrow (an svg
// inside TooltipContent) to match so it doesn't fall back to the theme color.
const TOOLTIP_DARK =
  "border-0 bg-[#2d2d2d] text-white [&_svg]:bg-[#2d2d2d] [&_svg]:fill-[#2d2d2d]";

// Drop intent disambiguation. We model each row as a droppable. A pointer in
// the top band means "insert before this node", the bottom band means "insert
// after", and (for folders only) the middle band means "drop into the folder".
type DropIntent = { kind: "before" } | { kind: "after" } | { kind: "into" };

interface DropTarget {
  overId: string;
  intent: DropIntent;
}

// Walk a node's subtree, collecting the ids of the node and all descendants.
// Used as the client-side cycle guard so a folder can never be dropped onto
// itself or one of its own descendants.
function collectSubtreeIds(node: NotebookTreeNode): Set<string> {
  const ids = new Set<string>();
  const walk = (n: NotebookTreeNode) => {
    ids.add(n.id);
    for (const child of n.children) {
      walk(child);
    }
  };
  walk(node);
  return ids;
}

// Find a node anywhere in the tree by id.
function findNode(
  nodes: NotebookTreeNode[],
  id: string,
): NotebookTreeNode | null {
  for (const node of nodes) {
    if (node.id === id) {
      return node;
    }
    const found = findNode(node.children, id);
    if (found) {
      return found;
    }
  }
  return null;
}

// Find the sibling list (and the parent folder id) that contains `id`.
function findSiblings(
  nodes: NotebookTreeNode[],
  id: string,
  parentId: string | null = null,
): { siblings: NotebookTreeNode[]; parentId: string | null } | null {
  if (nodes.some((n) => n.id === id)) {
    return { siblings: nodes, parentId };
  }
  for (const node of nodes) {
    const found = findSiblings(node.children, id, node.id);
    if (found) {
      return found;
    }
  }
  return null;
}

function expandedStorageKey(accountId: string | null): string {
  return `tradstry-notebook-tree-expanded:${accountId ?? "none"}`;
}

function loadExpanded(accountId: string | null): Set<string> {
  if (typeof window === "undefined") {
    return new Set();
  }
  try {
    const raw = window.localStorage.getItem(expandedStorageKey(accountId));
    if (!raw) {
      return new Set();
    }
    const parsed = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      return new Set(parsed.filter((v): v is string => typeof v === "string"));
    }
  } catch {
    // Ignore malformed persisted state and start collapsed.
  }
  return new Set();
}

interface TreeContext {
  accountId: string | null;
  selectedNoteId: string | null;
  trades: JournalEntry[];
  deletingNoteId: string | null;
  expanded: Set<string>;
  onToggleExpand: (folderId: string) => void;
  onSelectNote: (id: string) => void;
  onDeleteNote: (id: string) => void;
  onCreateNoteInFolder: (folderId: string | null) => void;
  onCreateFolder: (parentFolderId: string | null) => void;
  // Drag-and-drop state. `activeId` is the node currently being dragged;
  // `dropTarget` is the resolved drop intent (already cycle-guarded) used to
  // render the visual affordance (ring for "into", insertion bar for reorder).
  activeId: string | null;
  dropTarget: DropTarget | null;
}

export function NotebookTree({
  accountId,
  nodes,
  selectedNoteId,
  trades,
  deletingNoteId,
  onSelectNote,
  onDeleteNote,
  onCreateNoteInFolder,
  onCreateFolder,
}: {
  accountId: string | null;
  nodes: NotebookTreeNode[];
  selectedNoteId: string | null;
  trades: JournalEntry[];
  deletingNoteId: string | null;
  onSelectNote: (id: string) => void;
  onDeleteNote: (id: string) => void;
  onCreateNoteInFolder: (folderId: string | null) => void;
  onCreateFolder: (parentFolderId: string | null) => void;
}) {
  const [expanded, setExpanded] = useState<Set<string>>(() =>
    loadExpanded(accountId),
  );

  // Re-hydrate expansion state when the account changes (different storage key).
  useEffect(() => {
    setExpanded(loadExpanded(accountId));
  }, [accountId]);

  const persist = useCallback(
    (next: Set<string>) => {
      if (typeof window === "undefined") {
        return;
      }
      try {
        window.localStorage.setItem(
          expandedStorageKey(accountId),
          JSON.stringify([...next]),
        );
      } catch {
        // Storage may be unavailable (private mode); ignore.
      }
    },
    [accountId],
  );

  const onToggleExpand = useCallback(
    (folderId: string) => {
      setExpanded((current) => {
        const next = new Set(current);
        if (next.has(folderId)) {
          next.delete(folderId);
        } else {
          next.add(folderId);
        }
        persist(next);
        return next;
      });
    },
    [persist],
  );

  const moveNode = useMoveNotebookNode();

  const [activeId, setActiveId] = useState<string | null>(null);
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);

  // Require a small drag distance before activating so plain clicks still
  // select notes / toggle chevrons / focus the rename input.
  const sensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 5 } }),
  );

  // Resolve a drag event into a cycle-guarded drop target, or null if the
  // drop is illegal (onto itself, into its own descendant, or no-op).
  const resolveDropTarget = useCallback(
    (event: DragMoveEvent | DragEndEvent): DropTarget | null => {
      const draggedId = String(event.active.id);
      const over = event.over;
      if (!over) {
        return null;
      }
      const overId = String(over.id);

      // Root drop zone: move to top level, appended at the end.
      if (overId === ROOT_DROPPABLE_ID) {
        return { overId, intent: { kind: "into" } };
      }

      if (overId === draggedId) {
        return null;
      }

      const overNode = findNode(nodes, overId);
      if (!overNode) {
        return null;
      }

      // Cycle guard: a folder may not be dropped onto itself or a descendant.
      const draggedNode = findNode(nodes, draggedId);
      if (draggedNode && draggedNode.type === "FOLDER") {
        const subtree = collectSubtreeIds(draggedNode);
        if (subtree.has(overId)) {
          return null;
        }
      }

      // Disambiguate intent from pointer position within the over rect.
      const rect = over.rect;
      const pointerY =
        event.activatorEvent && "clientY" in event.activatorEvent
          ? (event.delta?.y ?? 0) +
            (event.activatorEvent as PointerEvent).clientY
          : rect.top + rect.height / 2;
      const fraction = (pointerY - rect.top) / rect.height;

      if (overNode.type === "FOLDER") {
        // Top 25% -> before, bottom 25% -> after, middle -> into folder.
        if (fraction < 0.25) {
          return { overId, intent: { kind: "before" } };
        }
        if (fraction > 0.75) {
          return { overId, intent: { kind: "after" } };
        }
        return { overId, intent: { kind: "into" } };
      }

      // Notes: top half -> before, bottom half -> after.
      return {
        overId,
        intent: { kind: fraction < 0.5 ? "before" : "after" },
      };
    },
    [nodes],
  );

  const handleDragStart = useCallback((event: DragStartEvent) => {
    setActiveId(String(event.active.id));
  }, []);

  const handleDragMove = useCallback(
    (event: DragMoveEvent) => {
      setDropTarget(resolveDropTarget(event));
    },
    [resolveDropTarget],
  );

  const handleDragCancel = useCallback(() => {
    setActiveId(null);
    setDropTarget(null);
  }, []);

  const handleDragEnd = useCallback(
    (event: DragEndEvent) => {
      const draggedId = String(event.active.id);
      const target = resolveDropTarget(event);
      setActiveId(null);
      setDropTarget(null);

      if (!target || !accountId) {
        return;
      }

      const draggedNode = findNode(nodes, draggedId);
      if (!draggedNode) {
        return;
      }

      let newParentFolderId: string | null;
      let newSortOrder: number;

      if (target.overId === ROOT_DROPPABLE_ID) {
        // Append to the end of the root sibling group.
        newParentFolderId = null;
        newSortOrder = nodes.length;
      } else if (target.intent.kind === "into") {
        // Reparent into the target folder, appended at the end.
        const folder = findNode(nodes, target.overId);
        newParentFolderId = target.overId;
        newSortOrder = folder ? folder.children.length : 0;
      } else {
        // Reorder relative to a sibling within the target's parent group.
        const group = findSiblings(nodes, target.overId);
        if (!group) {
          return;
        }
        newParentFolderId = group.parentId;
        const index = group.siblings.findIndex((n) => n.id === target.overId);
        newSortOrder = target.intent.kind === "before" ? index : index + 1;
      }

      // Skip pure no-ops (same parent + already at that slot).
      if (
        draggedNode.parentId === newParentFolderId &&
        draggedNode.sortOrder === newSortOrder
      ) {
        return;
      }

      moveNode.mutate({
        accountId,
        nodeId: draggedId,
        nodeType: draggedNode.type,
        newParentFolderId,
        newSortOrder,
      });
    },
    [accountId, moveNode, nodes, resolveDropTarget],
  );

  const ctx: TreeContext = {
    accountId,
    selectedNoteId,
    trades,
    deletingNoteId,
    expanded,
    onToggleExpand,
    onSelectNote,
    onDeleteNote,
    onCreateNoteInFolder,
    onCreateFolder,
    activeId,
    dropTarget,
  };

  if (nodes.length === 0) {
    return (
      <div className="rounded-xl border border-dashed border-border px-4 py-6 text-center text-sm leading-6 text-muted-foreground">
        No notes yet.
      </div>
    );
  }

  return (
    <DndContext
      sensors={sensors}
      collisionDetection={pointerWithin}
      onDragStart={handleDragStart}
      onDragMove={handleDragMove}
      onDragEnd={handleDragEnd}
      onDragCancel={handleDragCancel}
    >
      <div className="space-y-1">
        {nodes.map((node) => (
          <TreeNode key={node.id} node={node} depth={0} ctx={ctx} />
        ))}
        <RootDropZone active={activeId !== null} ctx={ctx} />
      </div>
    </DndContext>
  );
}

const ROOT_DROPPABLE_ID = "__notebook_root__";

// A thin drop zone at the bottom of the tree for moving a node to the root
// level (parent = null), appended at the end.
function RootDropZone({ active, ctx }: { active: boolean; ctx: TreeContext }) {
  const { setNodeRef, isOver } = useDroppable({ id: ROOT_DROPPABLE_ID });
  const highlighted = isOver && ctx.dropTarget?.overId === ROOT_DROPPABLE_ID;

  if (!active) {
    return null;
  }

  return (
    <div
      ref={setNodeRef}
      className={cn(
        "mt-1 rounded-lg border border-dashed px-3 py-2 text-center text-xs transition-colors",
        highlighted
          ? "border-foreground bg-accent text-foreground"
          : "border-border text-muted-foreground",
      )}
    >
      Move to top level
    </div>
  );
}

export function TreeNode({
  node,
  depth,
  ctx,
}: {
  node: NotebookTreeNode;
  depth: number;
  ctx: TreeContext;
}) {
  if (node.type === "FOLDER") {
    return <FolderRow node={node} depth={depth} ctx={ctx} />;
  }
  return <NoteRow node={node} depth={depth} ctx={ctx} />;
}

function FolderRow({
  node,
  depth,
  ctx,
}: {
  node: NotebookTreeNode;
  depth: number;
  ctx: TreeContext;
}) {
  const renameFolder = useRenameNotebookFolder();
  const deleteFolder = useDeleteNotebookFolder();
  const [confirming, setConfirming] = useState(false);
  const [isRenaming, setIsRenaming] = useState(false);
  const [draftName, setDraftName] = useState(node.name);
  const inputRef = useRef<HTMLInputElement>(null);
  const committedRef = useRef(false);

  const isExpanded = ctx.expanded.has(node.id);
  const isDeleting = deleteFolder.isPending;

  const {
    attributes,
    listeners,
    setNodeRef: setDragRef,
    isDragging,
  } = useDraggable({ id: node.id });
  const { setNodeRef: setDropRef } = useDroppable({ id: node.id });
  const setRowRef = useCallback(
    (el: HTMLElement | null) => {
      setDragRef(el);
      setDropRef(el);
    },
    [setDragRef, setDropRef],
  );

  // Resolve the drop affordance for this row from the shared drop target.
  const target = ctx.dropTarget?.overId === node.id ? ctx.dropTarget : null;
  const showInto = target?.intent.kind === "into";
  const showBefore = target?.intent.kind === "before";
  const showAfter = target?.intent.kind === "after";

  useEffect(() => {
    if (isRenaming) {
      // Focus + select once the input mounts.
      const id = window.requestAnimationFrame(() => {
        inputRef.current?.focus();
        inputRef.current?.select();
      });
      return () => window.cancelAnimationFrame(id);
    }
  }, [isRenaming]);

  const startRename = () => {
    setDraftName(node.name);
    committedRef.current = false;
    setIsRenaming(true);
  };

  const cancelRename = () => {
    committedRef.current = true;
    setIsRenaming(false);
    setDraftName(node.name);
  };

  const commitRename = () => {
    if (committedRef.current) {
      return;
    }
    committedRef.current = true;
    setIsRenaming(false);
    const trimmed = draftName.trim();
    if (trimmed.length === 0 || trimmed === node.name) {
      setDraftName(node.name);
      return;
    }
    renameFolder.mutate({ id: node.id, name: trimmed });
  };

  return (
    <div>
      <div
        ref={setRowRef}
        className={cn(
          "group relative flex items-center gap-1 rounded-lg py-1.5 pr-1 transition-colors hover:bg-accent",
          isDragging && "opacity-40",
          showInto && "ring-2 ring-inset ring-ring bg-accent",
        )}
        style={{ paddingLeft: depth * INDENT_STEP + INDENT_BASE }}
      >
        {showBefore && <InsertionLine position="top" />}
        {showAfter && <InsertionLine position="bottom" />}
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="size-6 shrink-0 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
          onClick={() => ctx.onToggleExpand(node.id)}
          aria-label={isExpanded ? "Collapse folder" : "Expand folder"}
        >
          <HugeiconsIcon
            icon={isExpanded ? ArrowDown01Icon : ArrowRight01Icon}
            strokeWidth={2}
          />
        </Button>
        <button
          type="button"
          {...attributes}
          {...listeners}
          className="flex size-6 shrink-0 cursor-grab items-center justify-center text-muted-foreground active:cursor-grabbing"
          aria-label="Drag folder"
        >
          <HugeiconsIcon icon={Folder01Icon} strokeWidth={2} />
        </button>

        {isRenaming ? (
          <input
            ref={inputRef}
            value={draftName}
            onChange={(event) => setDraftName(event.target.value)}
            onBlur={commitRename}
            onKeyDown={(event) => {
              if (event.key === "Enter") {
                event.preventDefault();
                commitRename();
              } else if (event.key === "Escape") {
                event.preventDefault();
                cancelRename();
              }
            }}
            className="min-w-0 flex-1 rounded-md border border-input bg-popover px-2 py-0.5 text-sm text-foreground outline-none focus:border-ring"
          />
        ) : (
          <button
            type="button"
            className="min-w-0 flex-1 truncate text-left text-sm font-medium text-foreground"
            onClick={() => ctx.onToggleExpand(node.id)}
            onDoubleClick={startRename}
          >
            {node.name}
          </button>
        )}

        {!isRenaming && (
          <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity group-hover:opacity-100 has-[[aria-expanded=true]]:opacity-100">
            <DropdownMenu>
              <Tooltip>
                <TooltipTrigger asChild>
                  <DropdownMenuTrigger asChild>
                    <Button
                      type="button"
                      variant="ghost"
                      size="icon-sm"
                      className="size-6 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground aria-expanded:bg-accent"
                      aria-label="More actions"
                    >
                      <MoreHorizontalIcon />
                    </Button>
                  </DropdownMenuTrigger>
                </TooltipTrigger>
                <TooltipContent side="bottom" className={TOOLTIP_DARK}>
                  Delete, duplicate, and more…
                </TooltipContent>
              </Tooltip>
              <DropdownMenuContent align="start" side="bottom" className="w-48">
                <DropdownMenuItem onClick={startRename}>
                  <RenameIcon />
                  <span>Rename</span>
                </DropdownMenuItem>
                <DropdownMenuItem onClick={() => ctx.onCreateFolder(node.id)}>
                  <HugeiconsIcon icon={FolderAddIcon} strokeWidth={2} />
                  <span>New subfolder</span>
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  variant="destructive"
                  onClick={() => setConfirming(true)}
                >
                  <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                  <span>Delete</span>
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>

            <Tooltip>
              <TooltipTrigger asChild>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-sm"
                  className="size-6 rounded-md text-muted-foreground hover:bg-accent hover:text-foreground"
                  aria-label="Add a note inside"
                  onClick={() => ctx.onCreateNoteInFolder(node.id)}
                >
                  <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" className={TOOLTIP_DARK}>
                Add a note inside
              </TooltipContent>
            </Tooltip>
          </div>
        )}
      </div>

      <Dialog open={confirming} onOpenChange={setConfirming}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete folder?</DialogTitle>
            <DialogDescription>
              This permanently deletes the folder and everything inside it.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setConfirming(false)}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={isDeleting}
              onClick={() => {
                if (ctx.accountId) {
                  deleteFolder.mutate({
                    id: node.id,
                    accountId: ctx.accountId,
                  });
                }
                setConfirming(false);
              }}
            >
              {isDeleting ? "Deleting..." : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {isExpanded && node.children.length > 0 && (
        <div className="space-y-1">
          {node.children.map((child) => (
            <TreeNode key={child.id} node={child} depth={depth + 1} ctx={ctx} />
          ))}
        </div>
      )}
    </div>
  );
}

function NoteRow({
  node,
  depth,
  ctx,
}: {
  node: NotebookTreeNode;
  depth: number;
  ctx: TreeContext;
}) {
  const [confirming, setConfirming] = useState(false);
  const isSelected = ctx.selectedNoteId === node.id;
  const isDeleting = ctx.deletingNoteId === node.id;
  const tradeIds = node.note?.tradeIds ?? [];
  const linked =
    tradeIds.length > 0
      ? ctx.trades.filter((t) => tradeIds.includes(t.id))
      : [];

  const {
    listeners,
    setNodeRef: setDragRef,
    isDragging,
  } = useDraggable({ id: node.id });
  const { setNodeRef: setDropRef } = useDroppable({ id: node.id });
  const setRowRef = useCallback(
    (el: HTMLElement | null) => {
      setDragRef(el);
      setDropRef(el);
    },
    [setDragRef, setDropRef],
  );

  const target = ctx.dropTarget?.overId === node.id ? ctx.dropTarget : null;
  const showBefore = target?.intent.kind === "before";
  const showAfter = target?.intent.kind === "after";

  return (
    <div
      ref={setRowRef}
      {...listeners}
      className={cn(
        "group relative flex cursor-grab items-center gap-1.5 rounded-md py-1.5 pr-1 transition-colors active:cursor-grabbing",
        isSelected
          ? "bg-accent font-medium text-foreground"
          : "text-muted-foreground hover:bg-accent/70",
        isDragging && "opacity-40",
      )}
      style={{ paddingLeft: depth * INDENT_STEP + INDENT_BASE }}
    >
      {showBefore && <InsertionLine position="top" />}
      {showAfter && <InsertionLine position="bottom" />}
      <span
        className={cn(
          "flex size-6 shrink-0 items-center justify-center",
          isSelected ? "text-muted-foreground" : "text-muted-foreground",
        )}
        aria-hidden="true"
      >
        <HugeiconsIcon icon={Note01Icon} strokeWidth={2} className="size-4" />
      </span>
      <button
        type="button"
        className="flex min-w-0 flex-1 items-center gap-2 text-left"
        onClick={() => ctx.onSelectNote(node.id)}
      >
        <span className="min-w-0 flex-1">
          <span className="block truncate text-[0.9375rem]">{node.name}</span>
          {linked.length > 0 && (
            <span className="mt-1 flex flex-wrap gap-1">
              {linked.map((t) => (
                <span
                  key={t.id}
                  className={cn(
                    "inline-flex items-center gap-1 rounded-full px-1.5 py-0 text-[0.6rem] font-medium",
                    t.status === "profit"
                      ? "bg-emerald-50 text-emerald-700 dark:bg-emerald-950/50 dark:text-emerald-300"
                      : "bg-rose-50 text-rose-700 dark:bg-rose-950/50 dark:text-rose-300",
                  )}
                >
                  {t.symbol}
                  <span className="opacity-60">
                    {t.tradeType === "long" ? "L" : "S"}
                  </span>
                </span>
              ))}
            </span>
          )}
        </span>
      </button>

      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="size-6 shrink-0 rounded-md text-muted-foreground opacity-0 transition-opacity hover:bg-muted hover:text-foreground group-hover:opacity-100 aria-expanded:bg-muted aria-expanded:opacity-100"
                aria-label="More actions"
                disabled={isDeleting}
              >
                <MoreHorizontalIcon />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent side="bottom" className={TOOLTIP_DARK}>
            Delete, duplicate, and more…
          </TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="start" side="bottom" className="w-48">
          <DropdownMenuItem
            variant="destructive"
            onClick={() => setConfirming(true)}
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
            <span>Delete</span>
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>

      <Dialog open={confirming} onOpenChange={setConfirming}>
        <DialogContent className="sm:max-w-md">
          <DialogHeader>
            <DialogTitle>Delete note?</DialogTitle>
            <DialogDescription>
              This permanently deletes this note and its notebook images.
            </DialogDescription>
          </DialogHeader>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setConfirming(false)}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              disabled={isDeleting}
              onClick={() => {
                ctx.onDeleteNote(node.id);
                setConfirming(false);
              }}
            >
              {isDeleting ? "Deleting..." : "Delete"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// A 2px insertion bar rendered at the top or bottom edge of a row to signal a
// reorder drop (insert-before / insert-after).
function InsertionLine({ position }: { position: "top" | "bottom" }) {
  return (
    <span
      aria-hidden="true"
      className={cn(
        "pointer-events-none absolute inset-x-1 h-0.5 rounded-full bg-foreground",
        position === "top" ? "-top-px" : "-bottom-px",
      )}
    />
  );
}

// Inline three-dots "more actions" glyph (plain, non-circled — matches Notion).
function MoreHorizontalIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="currentColor"
      aria-hidden="true"
    >
      <circle cx="5" cy="12" r="1.6" />
      <circle cx="12" cy="12" r="1.6" />
      <circle cx="19" cy="12" r="1.6" />
    </svg>
  );
}

// Inline pencil/rename SVG (no dedicated rename glyph imported elsewhere).
function RenameIcon() {
  return (
    <svg
      width="18"
      height="18"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4 12.5-12.5z" />
    </svg>
  );
}
