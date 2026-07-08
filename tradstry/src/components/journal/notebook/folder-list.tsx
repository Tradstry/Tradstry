import { useState } from "react";
import {
  CheckIcon,
  FolderIcon,
  NotebookIcon,
  PencilSimpleIcon,
  PlusIcon,
  StackIcon,
  TrashIcon,
  TrayIcon,
} from "@phosphor-icons/react";
import { ScrollArea } from "../../user-interface";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { type Folder, type Note } from "./types";

function Row({
  icon,
  label,
  count,
  active,
  onClick,
  children,
}: {
  icon: React.ReactNode;
  label: React.ReactNode;
  count: number;
  active: boolean;
  onClick: () => void;
  children?: React.ReactNode;
}) {
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
      className={cn(
        "group relative flex cursor-pointer items-center gap-2 rounded-lg px-2.5 py-1.5 text-sm outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40",
        active
          ? "bg-primary/10 text-foreground"
          : "text-zinc-600 hover:bg-muted/60 dark:text-zinc-300",
      )}
    >
      {active && (
        <span className="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-primary" />
      )}
      <span className="shrink-0 text-muted-foreground">{icon}</span>
      <span className="flex-1 truncate">{label}</span>
      {children ?? (
        <span className="text-xs tabular-nums text-muted-foreground">
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
}: {
  folder: Folder;
  count: number;
  active: boolean;
  onSelect: () => void;
  onRename: (name: string) => void;
  onDelete: () => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(folder.name);

  const commit = () => {
    const name = draft.trim();
    if (name && name !== folder.name) onRename(name);
    setEditing(false);
  };

  if (editing) {
    return (
      <div className="flex items-center gap-1 rounded-lg px-2.5 py-1">
        <FolderIcon size={15} className="shrink-0 text-muted-foreground" />
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
        <Button type="button" size="icon-xs" variant="ghost" onClick={commit}>
          <CheckIcon size={13} />
        </Button>
      </div>
    );
  }

  return (
    <Row
      icon={<FolderIcon size={15} />}
      label={folder.name}
      count={count}
      active={active}
      onClick={onSelect}
    >
      <div className="flex items-center gap-0.5">
        <span className="text-xs tabular-nums text-muted-foreground group-hover:hidden">
          {count}
        </span>
        <div className="hidden items-center group-hover:flex">
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label="Rename folder"
            onClick={(e) => {
              e.stopPropagation();
              setDraft(folder.name);
              setEditing(true);
            }}
          >
            <PencilSimpleIcon size={13} />
          </Button>
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label="Delete folder"
            className="hover:bg-destructive/10 hover:text-destructive"
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
          >
            <TrashIcon size={13} />
          </Button>
        </div>
      </div>
    </Row>
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
}: {
  folders: Folder[];
  notes: Note[];
  active: string;
  onSelect: (v: string) => void;
  onCreateFolder: (name: string) => void;
  onRenameFolder: (id: string, name: string) => void;
  onDeleteFolder: (id: string) => void;
}) {
  const [newName, setNewName] = useState("");
  const uncatCount = notes.filter((n) => n.folderId == null).length;

  return (
    <div className="flex w-52 shrink-0 flex-col border-r border-zinc-200/70 dark:border-zinc-800/70">
      <div className="flex items-center gap-2 px-3 py-3">
        <NotebookIcon size={16} weight="fill" className="text-blue-500" />
        <span className="text-sm font-semibold">Notebook</span>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        <div className="space-y-0.5 px-2 pb-2">
          <Row
            icon={<StackIcon size={15} />}
            label="All notes"
            count={notes.length}
            active={active === "all"}
            onClick={() => onSelect("all")}
          />
          {folders.map((f) => (
            <FolderRow
              key={f.id}
              folder={f}
              count={notes.filter((n) => n.folderId === f.id).length}
              active={active === f.id}
              onSelect={() => onSelect(f.id)}
              onRename={(name) => onRenameFolder(f.id, name)}
              onDelete={() => onDeleteFolder(f.id)}
            />
          ))}
          {uncatCount > 0 && (
            <Row
              icon={<TrayIcon size={15} />}
              label="Uncategorized"
              count={uncatCount}
              active={active === "uncat"}
              onClick={() => onSelect("uncat")}
            />
          )}
        </div>
      </ScrollArea>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          const name = newName.trim();
          if (!name) return;
          onCreateFolder(name);
          setNewName("");
        }}
        className="flex items-center gap-2 border-t border-zinc-200/70 p-2 dark:border-zinc-800/70"
      >
        <Input
          value={newName}
          onChange={(e) => setNewName(e.target.value)}
          placeholder="New folder…"
          className="h-8 flex-1 text-sm"
        />
        <Button
          type="submit"
          size="icon-sm"
          variant="ghost"
          disabled={!newName.trim()}
          aria-label="Add folder"
        >
          <PlusIcon size={15} />
        </Button>
      </form>
    </div>
  );
}
