"use client";

import {
  Add01Icon,
  Delete02Icon,
  Folder01Icon,
  InboxIcon,
  Layers01Icon,
  PencilEdit01Icon,
  SparklesIcon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { type DragEvent, useState } from "react";
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
import { NOTE_DND_TYPE } from "./dnd";

function Row({
  icon,
  label,
  count,
  active,
  onClick,
  onDropNote,
  children,
}: {
  icon: React.ReactNode;
  label: React.ReactNode;
  count: number;
  active: boolean;
  onClick: () => void;
  onDropNote?: (noteId: string) => void;
  children?: React.ReactNode;
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
      {children ?? (
        <span className="flex w-10 shrink-0 justify-end text-xs tabular-nums text-muted-foreground">
          {count}
        </span>
      )}
    </div>
  );
}

function FolderRow({
  folder,
  count,
  active,
  onSelect,
  onRename,
  onDelete,
  onDropNote,
}: {
  folder: NotebookFolder;
  count: number;
  active: boolean;
  onSelect: () => void;
  onRename: (name: string) => void;
  onDelete: () => void;
  onDropNote: (noteId: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(folder.name);
  const [confirmOpen, setConfirmOpen] = useState(false);

  const commit = () => {
    const name = draft.trim();
    if (name && name !== folder.name) onRename(name);
    setEditing(false);
  };

  if (editing) {
    return (
      <div className="flex items-center gap-1 rounded-lg px-2.5 py-1">
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
    <Row
      icon={<HugeiconsIcon icon={Folder01Icon} size={15} strokeWidth={2} />}
      label={folder.name}
      count={count}
      active={active}
      onClick={onSelect}
      onDropNote={onDropNote}
    >
      <div className="relative flex w-14 shrink-0 items-center justify-end">
        <span className="text-xs tabular-nums text-muted-foreground transition-opacity duration-150 group-hover:opacity-0 group-focus-within:opacity-0">
          {count}
        </span>
        <div className="absolute inset-y-0 right-0 flex items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100">
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label="Rename folder"
            className="size-6"
            onClick={(e) => {
              e.stopPropagation();
              setDraft(folder.name);
              setEditing(true);
            }}
          >
            <HugeiconsIcon icon={PencilEdit01Icon} size={13} strokeWidth={2} />
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
                <HugeiconsIcon icon={Delete02Icon} size={13} strokeWidth={2} />
              </Button>
            </DialogTrigger>
            <DialogContent className="sm:max-w-sm">
              <DialogHeader>
                <DialogTitle>Delete folder</DialogTitle>
                <DialogDescription>
                  Delete &quot;{folder.name}&quot; and every note inside it?
                  This can&apos;t be undone.
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
                    onDelete();
                    setConfirmOpen(false);
                  }}
                >
                  Delete folder
                </Button>
              </DialogFooter>
            </DialogContent>
          </Dialog>
        </div>
      </div>
    </Row>
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
}: {
  folders: NotebookFolder[];
  notes: NotebookNote[];
  active: string;
  onSelect: (v: string) => void;
  onCreateFolder: (name: string) => void;
  onRenameFolder: (id: string, name: string) => void;
  onDeleteFolder: (id: string) => void;
  onMoveNote: (noteId: string, folderId: string | null) => void;
}) {
  const uncatCount = notes.filter((n) => n.folderId == null).length;
  const systemFolder = folders.find((f) => f.isSystem);
  // Kept out of the list below so it can never render a rename or delete affordance.
  const userFolders = folders.filter((f) => !f.isSystem);

  return (
    <div className="flex h-full w-52 shrink-0 flex-col border-r border-border/60">
      <div className="flex h-13 items-center justify-between gap-2 px-3">
        <span className="text-base font-semibold">Notebook</span>
        <NewFolderDialog onCreateFolder={onCreateFolder} />
      </div>

      <ScrollArea className="min-h-0 flex-1 [&>[data-radix-scroll-area-viewport]>div]:!block">
        <div className="space-y-0.5 px-2 pb-2">
          {systemFolder ? (
            <Row
              icon={
                <HugeiconsIcon
                  icon={SparklesIcon}
                  size={15}
                  strokeWidth={2}
                  className="text-primary"
                />
              }
              label={systemFolder.name}
              count={notes.filter((n) => n.folderId === systemFolder.id).length}
              active={active === systemFolder.id}
              onClick={() => onSelect(systemFolder.id)}
              onDropNote={(noteId) => onMoveNote(noteId, systemFolder.id)}
            />
          ) : null}
          {/* Pinned above All notes and always a drop target: dragging a note here
              clears its folder. */}
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
          {userFolders.map((f) => (
            <FolderRow
              key={f.id}
              folder={f}
              count={notes.filter((n) => n.folderId === f.id).length}
              active={active === f.id}
              onSelect={() => onSelect(f.id)}
              onRename={(name) => onRenameFolder(f.id, name)}
              onDelete={() => onDeleteFolder(f.id)}
              onDropNote={(noteId) => onMoveNote(noteId, f.id)}
            />
          ))}
        </div>
      </ScrollArea>
    </div>
  );
}
