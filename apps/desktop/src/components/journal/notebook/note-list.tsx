import { SidebarSimpleIcon, TrashIcon } from "@phosphor-icons/react";
import { ScrollArea } from "../../user-interface";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { AddButton } from "./add-button";
import { cn } from "@/lib/utils";
import { notePreview } from "@tradstry/notebook-core";
import { NOTE_DND_TYPE } from "./dnd";
import type { Note } from "./types";

export function NoteList({
  title,
  notes,
  selectedId,
  onSelect,
  onCreateNote,
  onDeleteNote,
  sidebarCollapsed,
  onToggleSidebar,
}: {
  title: string;
  notes: Note[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreateNote: () => void;
  onDeleteNote: (id: string) => void;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}) {
  return (
    <div className="flex w-72 shrink-0 flex-col border-r border-zinc-200/70 dark:border-zinc-800/70">
      <div className="flex h-13 items-center gap-2 px-3">
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label={sidebarCollapsed ? "Show folders" : "Hide folders"}
              aria-expanded={!sidebarCollapsed}
              onClick={onToggleSidebar}
              className="-ml-1 shrink-0 text-zinc-500 dark:text-zinc-400"
            >
              <SidebarSimpleIcon size={16} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {sidebarCollapsed ? "Show folders" : "Hide folders"} &nbsp;⌘\
          </TooltipContent>
        </Tooltip>
        <span className="flex-1 truncate text-sm font-semibold">{title}</span>
        <AddButton label="New note" onClick={onCreateNote} />
      </div>

      <ScrollArea className="min-h-0 flex-1">
        {notes.length === 0 ? (
          <p className="px-3 py-10 text-center text-xs text-muted-foreground">
            No notes here yet.
          </p>
        ) : (
          <div className="space-y-0.5 px-2 pb-2">
            {notes.map((note) => {
              const selected = note.id === selectedId;
              const preview = notePreview(note.documentJson);
              return (
                <div
                  key={note.id}
                  role="button"
                  tabIndex={0}
                  draggable
                  onDragStart={(e) => {
                    e.dataTransfer.setData(NOTE_DND_TYPE, note.id);
                    e.dataTransfer.effectAllowed = "move";
                  }}
                  onClick={() => onSelect(note.id)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" || e.key === " ") {
                      e.preventDefault();
                      onSelect(note.id);
                    }
                  }}
                  className={cn(
                    "group relative cursor-pointer rounded-lg px-3 py-2.5 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40",
                    selected ? "bg-primary/10" : "hover:bg-muted/60",
                  )}
                >
                  <div className="flex items-start justify-between gap-2">
                    <p
                      className={cn(
                        "flex-1 truncate text-sm font-medium",
                        selected
                          ? "text-foreground"
                          : "text-zinc-800 dark:text-zinc-100",
                      )}
                    >
                      {note.title || "Untitled"}
                    </p>
                    <Button
                      type="button"
                      size="icon-xs"
                      variant="ghost"
                      aria-label="Delete note"
                      className="-mr-1 -mt-0.5 shrink-0 opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
                      onClick={(e) => {
                        e.stopPropagation();
                        onDeleteNote(note.id);
                      }}
                    >
                      <TrashIcon size={13} />
                    </Button>
                  </div>
                  <p className="mt-0.5 line-clamp-2 text-xs text-muted-foreground">
                    {preview || "No additional text"}
                  </p>
                </div>
              );
            })}
          </div>
        )}
      </ScrollArea>
    </div>
  );
}
