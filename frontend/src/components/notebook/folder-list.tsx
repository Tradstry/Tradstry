"use client";

import {
  Add01Icon,
  ArrowRight01Icon,
  Delete02Icon,
  Folder01Icon,
  FolderAddIcon,
  InboxIcon,
  Layers01Icon,
  PencilEdit01Icon,
  SparklesIcon,
  StarIcon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type DragEvent, useMemo, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { NotebookFolder, NotebookNote } from "@/lib/types/notebook";
import { cn } from "@/lib/utils";
import { FOLDER_DND_TYPE, NOTE_DND_TYPE } from "./dnd";

const INDENT = 14;

type FolderNode = NotebookFolder & { children: FolderNode[] };

function buildTree(folders: NotebookFolder[]): FolderNode[] {
  const byId = new Map<string, FolderNode>(
    folders.map((f) => [f.id, { ...f, children: [] }]),
  );
  const roots: FolderNode[] = [];
  for (const node of byId.values()) {
    const parent = node.parentFolderId
      ? byId.get(node.parentFolderId)
      : undefined;
    if (parent) parent.children.push(node);
    else roots.push(node);
  }
  const sort = (nodes: FolderNode[]) => {
    nodes.sort((a, b) =>
      a.sortOrder !== b.sortOrder
        ? a.sortOrder - b.sortOrder
        : a.name.localeCompare(b.name),
    );
    for (const n of nodes) sort(n.children);
  };
  sort(roots);
  return roots;
}

/** A non-folder sidebar entry: System, Uncategorized, All notes. */
function Row({
  icon,
  label,
  count,
  active,
  onClick,
  onDropNote,
}: {
  icon: React.ReactNode;
  label: React.ReactNode;
  count: number;
  active: boolean;
  onClick: () => void;
  onDropNote?: (noteId: string) => void;
}) {
  const [dragOver, setDragOver] = useState(false);
  const dropProps = onDropNote
    ? {
        onDragOver: (e: DragEvent) => {
          if (!e.dataTransfer.types.includes(NOTE_DND_TYPE)) return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "move" as const;
          setDragOver(true);
        },
        onDragLeave: () => setDragOver(false),
        onDrop: (e: DragEvent) => {
          const noteId = e.dataTransfer.getData(NOTE_DND_TYPE);
          setDragOver(false);
          if (noteId) {
            e.preventDefault();
            onDropNote(noteId);
          }
        },
      }
    : {};

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onClick}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onClick();
        }
      }}
      {...dropProps}
      className={cn(
        "group relative flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-1.5 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40",
        active
          ? "bg-primary/10 text-foreground"
          : "text-foreground/80 hover:bg-muted/60",
        dragOver && "bg-primary/5 ring-2 ring-primary/60",
      )}
    >
      {active ? (
        <span className="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-primary" />
      ) : null}
      <span className="shrink-0 text-muted-foreground">{icon}</span>
      <span className="min-w-0 flex-1 truncate">{label}</span>
      <span className="flex w-10 shrink-0 justify-end text-xs tabular-nums text-muted-foreground">
        {count}
      </span>
    </div>
  );
}

function FolderRow({
  node,
  depth,
  active,
  expanded,
  noteCount,
  onToggle,
  onSelect,
  onRename,
  onDelete,
  onCreateSubfolder,
  onDropNote,
  onDropFolder,
}: {
  node: FolderNode;
  depth: number;
  active: string;
  expanded: Set<string>;
  noteCount: (folderId: string) => number;
  onToggle: (id: string) => void;
  onSelect: (id: string) => void;
  onRename: (id: string, name: string) => void;
  onDelete: (id: string) => void;
  onCreateSubfolder: (parentId: string, name: string) => void;
  onDropNote: (noteId: string, folderId: string) => void;
  onDropFolder: (folderId: string, newParentId: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(node.name);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [addingChild, setAddingChild] = useState(false);
  const [childName, setChildName] = useState("");
  const [dragOver, setDragOver] = useState(false);

  const hasChildren = node.children.length > 0;
  const isOpen = expanded.has(node.id);
  // The System folder is a real node now — it can hold children and spawn
  // subfolders — but it can't be renamed, deleted, or dragged.
  const isSystem = node.isSystem;

  const commit = () => {
    const name = draft.trim();
    if (name && name !== node.name) onRename(node.id, name);
    setEditing(false);
  };

  const commitChild = () => {
    const name = childName.trim();
    if (name) {
      onCreateSubfolder(node.id, name);
      if (!isOpen) onToggle(node.id);
    }
    setChildName("");
    setAddingChild(false);
  };

  if (editing) {
    return (
      <div
        className="flex items-center gap-1 rounded-lg px-2.5 py-1"
        style={{ paddingLeft: 10 + depth * INDENT }}
      >
        <HugeiconsIcon
          icon={Folder01Icon}
          size={15}
          strokeWidth={2}
          className="shrink-0 text-muted-foreground"
        />
        <Input
          autoFocus
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          onBlur={commit}
          onKeyDown={(e) => {
            if (e.key === "Enter") commit();
            if (e.key === "Escape") setEditing(false);
          }}
          className="h-6 flex-1 text-sm"
        />
        <Button type="button" size="icon-sm" variant="ghost" onClick={commit}>
          <HugeiconsIcon icon={Tick02Icon} size={13} strokeWidth={2} />
        </Button>
      </div>
    );
  }

  return (
    <>
      <div
        role="button"
        tabIndex={0}
        draggable={!isSystem}
        onDragStart={(e) => {
          if (isSystem) return;
          e.dataTransfer.setData(FOLDER_DND_TYPE, node.id);
          e.dataTransfer.effectAllowed = "move";
        }}
        onDragOver={(e) => {
          const t = e.dataTransfer.types;
          if (!t.includes(NOTE_DND_TYPE) && !t.includes(FOLDER_DND_TYPE))
            return;
          e.preventDefault();
          e.dataTransfer.dropEffect = "move";
          setDragOver(true);
        }}
        onDragLeave={() => setDragOver(false)}
        onDrop={(e) => {
          setDragOver(false);
          const noteId = e.dataTransfer.getData(NOTE_DND_TYPE);
          const folderId = e.dataTransfer.getData(FOLDER_DND_TYPE);
          if (noteId) {
            e.preventDefault();
            onDropNote(noteId, node.id);
          } else if (folderId && folderId !== node.id) {
            e.preventDefault();
            onDropFolder(folderId, node.id);
          }
        }}
        onClick={() => onSelect(node.id)}
        onKeyDown={(e) => {
          if (e.key === "Enter" || e.key === " ") {
            e.preventDefault();
            onSelect(node.id);
          }
        }}
        style={{ paddingLeft: 10 + depth * INDENT }}
        className={cn(
          "group relative flex cursor-pointer items-center gap-1.5 rounded-lg py-1.5 pr-2.5 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40",
          active === node.id
            ? "bg-primary/10 text-foreground"
            : "text-foreground/80 hover:bg-muted/60",
          dragOver && "bg-primary/5 ring-2 ring-primary/60",
        )}
      >
        {active === node.id ? (
          <span className="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-primary" />
        ) : null}
        <button
          type="button"
          aria-label={isOpen ? "Collapse" : "Expand"}
          onClick={(e) => {
            e.stopPropagation();
            if (hasChildren) onToggle(node.id);
          }}
          className={cn(
            "flex size-4 shrink-0 items-center justify-center text-muted-foreground",
            !hasChildren && "invisible",
          )}
        >
          <HugeiconsIcon
            icon={ArrowRight01Icon}
            size={14}
            strokeWidth={2}
            className={cn("transition-transform", isOpen && "rotate-90")}
          />
        </button>
        <HugeiconsIcon
          icon={isSystem ? SparklesIcon : Folder01Icon}
          size={15}
          strokeWidth={2}
          className={cn(
            "shrink-0",
            isSystem ? "text-primary" : "text-muted-foreground",
          )}
        />
        <span className="min-w-0 flex-1 truncate">{node.name}</span>

        <div className="relative flex shrink-0 items-center justify-end">
          <span className="w-5 text-right text-xs tabular-nums text-muted-foreground transition-opacity duration-150 group-hover:opacity-0 group-focus-within:opacity-0">
            {noteCount(node.id)}
          </span>
          {/* Actions overlay the label's right edge on hover, masked so text doesn't show through. */}
          <div className="absolute inset-y-0 right-0 flex items-center gap-0.5 rounded-lg bg-muted pl-2 opacity-0 shadow-[-8px_0_8px_-4px_var(--muted)] transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100">
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label="New subfolder"
              className="size-6"
              onClick={(e) => {
                e.stopPropagation();
                setChildName("");
                setAddingChild(true);
              }}
            >
              <HugeiconsIcon icon={FolderAddIcon} size={13} strokeWidth={2} />
            </Button>
            {!isSystem ? (
              <>
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  aria-label="Rename folder"
                  className="size-6"
                  onClick={(e) => {
                    e.stopPropagation();
                    setDraft(node.name);
                    setEditing(true);
                  }}
                >
                  <HugeiconsIcon
                    icon={PencilEdit01Icon}
                    size={13}
                    strokeWidth={2}
                  />
                </Button>
                <Dialog open={confirmOpen} onOpenChange={setConfirmOpen}>
                  <DialogTrigger asChild>
                    <Button
                      type="button"
                      size="icon-sm"
                      variant="ghost"
                      aria-label="Delete folder"
                      className="size-6 hover:bg-destructive/10 hover:text-destructive"
                      onClick={(e) => e.stopPropagation()}
                    >
                      <HugeiconsIcon
                        icon={Delete02Icon}
                        size={13}
                        strokeWidth={2}
                      />
                    </Button>
                  </DialogTrigger>
                  <DialogContent className="sm:max-w-sm">
                    <DialogHeader>
                      <DialogTitle>Delete folder</DialogTitle>
                      <DialogDescription>
                        Delete &quot;{node.name}&quot;
                        {hasChildren ? ", its subfolders" : ""} and every note
                        inside? This can&apos;t be undone.
                      </DialogDescription>
                    </DialogHeader>
                    <DialogFooter>
                      <Button
                        type="button"
                        variant="ghost"
                        onClick={() => setConfirmOpen(false)}
                      >
                        Cancel
                      </Button>
                      <Button
                        type="button"
                        variant="destructive"
                        onClick={() => {
                          onDelete(node.id);
                          setConfirmOpen(false);
                        }}
                      >
                        Delete folder
                      </Button>
                    </DialogFooter>
                  </DialogContent>
                </Dialog>
              </>
            ) : null}
          </div>
        </div>
      </div>

      {addingChild ? (
        <div
          className="flex items-center gap-1 py-1 pr-2.5"
          style={{ paddingLeft: 10 + (depth + 1) * INDENT }}
        >
          <HugeiconsIcon
            icon={Folder01Icon}
            size={15}
            strokeWidth={2}
            className="shrink-0 text-muted-foreground"
          />
          <Input
            autoFocus
            value={childName}
            placeholder="Subfolder name"
            onChange={(e) => setChildName(e.target.value)}
            onBlur={commitChild}
            onKeyDown={(e) => {
              if (e.key === "Enter") commitChild();
              if (e.key === "Escape") setAddingChild(false);
            }}
            className="h-6 flex-1 text-sm"
          />
        </div>
      ) : null}

      {isOpen
        ? node.children.map((child) => (
            <FolderRow
              key={child.id}
              node={child}
              depth={depth + 1}
              active={active}
              expanded={expanded}
              noteCount={noteCount}
              onToggle={onToggle}
              onSelect={onSelect}
              onRename={onRename}
              onDelete={onDelete}
              onCreateSubfolder={onCreateSubfolder}
              onDropNote={onDropNote}
              onDropFolder={onDropFolder}
            />
          ))
        : null}
    </>
  );
}

function NewFolderDialog({
  onCreateFolder,
}: {
  onCreateFolder: (name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");

  const dismiss = () => {
    setName("");
    setOpen(false);
  };

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Tooltip>
        <TooltipTrigger asChild>
          <DialogTrigger asChild>
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label="New folder"
              className="text-muted-foreground"
            >
              <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={2} />
            </Button>
          </DialogTrigger>
        </TooltipTrigger>
        <TooltipContent side="bottom">New folder</TooltipContent>
      </Tooltip>
      <DialogContent className="sm:max-w-sm">
        <form
          onSubmit={(e) => {
            e.preventDefault();
            const trimmed = name.trim();
            if (!trimmed) return;
            onCreateFolder(trimmed);
            dismiss();
          }}
        >
          <DialogHeader>
            <DialogTitle>New folder</DialogTitle>
            <DialogDescription>Give your folder a name.</DialogDescription>
          </DialogHeader>
          <Input
            autoFocus
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Folder name"
            className="my-4"
          />
          <DialogFooter>
            <Button type="button" variant="ghost" onClick={dismiss}>
              Cancel
            </Button>
            <Button type="submit" disabled={!name.trim()}>
              Create folder
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

export function FolderList({
  folders,
  notes,
  active,
  onSelect,
  onCreateFolder,
  onRenameFolder,
  onDeleteFolder,
  onMoveNote,
  onMoveFolder,
}: {
  folders: NotebookFolder[];
  notes: NotebookNote[];
  active: string;
  onSelect: (v: string) => void;
  onCreateFolder: (name: string, parentFolderId?: string | null) => void;
  onRenameFolder: (id: string, name: string) => void;
  onDeleteFolder: (id: string) => void;
  onMoveNote: (noteId: string, folderId: string | null) => void;
  onMoveFolder: (folderId: string, newParentId: string | null) => void;
}) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set());
  const toggle = (id: string) =>
    setExpanded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const uncatCount = notes.filter((n) => n.folderId == null).length;
  const starredCount = notes.filter((n) => n.isStarred).length;
  const noteCount = (folderId: string) =>
    notes.filter((n) => n.folderId === folderId).length;

  // System is part of the tree so its subfolders nest under it; it's pinned to
  // the top of the sidebar while the rest of the user's folders follow.
  const tree = useMemo(() => buildTree(folders), [folders]);
  const systemNode = tree.find((n) => n.isSystem);
  const userTree = tree.filter((n) => !n.isSystem);

  return (
    <div className="flex h-full w-60 shrink-0 flex-col border-r border-border/60">
      <div className="flex h-13 items-center justify-between gap-2 px-3">
        <span className="text-base font-semibold">Notebook</span>
        <NewFolderDialog
          onCreateFolder={(name) => onCreateFolder(name, null)}
        />
      </div>

      <ScrollArea className="min-h-0 flex-1 [&>[data-radix-scroll-area-viewport]>div]:!block">
        <div className="space-y-0.5 px-2 pb-2">
          {systemNode ? (
            <FolderRow
              node={systemNode}
              depth={0}
              active={active}
              expanded={expanded}
              noteCount={noteCount}
              onToggle={toggle}
              onSelect={onSelect}
              onRename={onRenameFolder}
              onDelete={onDeleteFolder}
              onCreateSubfolder={(parentId, name) =>
                onCreateFolder(name, parentId)
              }
              onDropNote={onMoveNote}
              onDropFolder={onMoveFolder}
            />
          ) : null}
          {/* Dropping a note here clears its folder. */}
          <Row
            icon={<HugeiconsIcon icon={InboxIcon} size={15} strokeWidth={2} />}
            label="Uncategorized"
            count={uncatCount}
            active={active === "uncat"}
            onClick={() => onSelect("uncat")}
            onDropNote={(noteId) => onMoveNote(noteId, null)}
          />
          <Row
            icon={
              <HugeiconsIcon icon={Layers01Icon} size={15} strokeWidth={2} />
            }
            label="All notes"
            count={notes.length}
            active={active === "all"}
            onClick={() => onSelect("all")}
          />
          {/* Virtual view of starred notes — they stay in their real folders. */}
          <Row
            icon={
              <HugeiconsIcon
                icon={StarIcon}
                size={15}
                strokeWidth={2}
                className="text-amber-500 [&_path]:fill-current"
              />
            }
            label="Favourites"
            count={starredCount}
            active={active === "starred"}
            onClick={() => onSelect("starred")}
          />
          {userTree.map((node) => (
            <FolderRow
              key={node.id}
              node={node}
              depth={0}
              active={active}
              expanded={expanded}
              noteCount={noteCount}
              onToggle={toggle}
              onSelect={onSelect}
              onRename={onRenameFolder}
              onDelete={onDeleteFolder}
              onCreateSubfolder={(parentId, name) =>
                onCreateFolder(name, parentId)
              }
              onDropNote={onMoveNote}
              onDropFolder={onMoveFolder}
            />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
