"use client";

import {
  Add01Icon,
  Delete02Icon,
  PinIcon,
  SidebarLeft01Icon,
  StarIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { notePreview } from "@tradstry/notebook-core";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@tradstry/app-ui/components/ui/tooltip";
import type { NotebookNote } from "@tradstry/app-ui/lib/types/notebook";
import { cn } from "@tradstry/app-ui/lib/utils";
import { NOTE_DND_TYPE } from "./dnd";

export function NoteList({
  title,
  notes,
  selectedId,
  onSelect,
  onCreateNote,
  onDeleteNote,
  onToggleStar,
  onTogglePin,
  sidebarCollapsed,
  onToggleSidebar,
}: {
  title: string;
  notes: NotebookNote[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreateNote: () => void;
  onDeleteNote: (id: string) => void;
  onToggleStar: (id: string, isStarred: boolean) => void;
  onTogglePin: (id: string, isPinned: boolean) => void;
  sidebarCollapsed: boolean;
  onToggleSidebar: () => void;
}) {
  return (
    <div className="flex w-72 shrink-0 flex-col border-r border-border/60">
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
              className="-ml-1 shrink-0 text-muted-foreground"
            >
              <HugeiconsIcon
                icon={SidebarLeft01Icon}
                size={16}
                strokeWidth={2}
              />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">
            {sidebarCollapsed ? "Show folders" : "Hide folders"}
          </TooltipContent>
        </Tooltip>
        <span className="flex-1 truncate text-sm font-semibold">{title}</span>
        <Tooltip>
          <TooltipTrigger asChild>
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label="New note"
              onClick={onCreateNote}
              className="shrink-0 text-muted-foreground"
            >
              <HugeiconsIcon icon={Add01Icon} size={16} strokeWidth={2} />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom">New note</TooltipContent>
        </Tooltip>
      </div>

      {/* Radix wraps viewport content in a `display: table` div, which sizes to content —
          so `truncate` never gets a width to work against. Force it back to block. */}
      <ScrollArea className="min-h-0 flex-1 [&>[data-radix-scroll-area-viewport]>div]:!block">
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
                        "flex min-w-0 flex-1 items-center gap-1.5 truncate text-sm font-medium",
                        selected ? "text-foreground" : "text-foreground/90",
                      )}
                    >
                      {note.isPinned ? (
                        <HugeiconsIcon
                          icon={PinIcon}
                          size={12}
                          strokeWidth={2}
                          className="shrink-0 -rotate-45 text-muted-foreground"
                        />
                      ) : null}
                      <span className="truncate">
                        {note.title || "Untitled"}
                      </span>
                    </p>
                    <div className="-mr-1 -mt-0.5 flex shrink-0 items-center">
                      {/* Star stays visible when set so Favourites membership reads at a glance. */}
                      <Button
                        type="button"
                        size="icon-sm"
                        variant="ghost"
                        aria-label={
                          note.isStarred ? "Unstar note" : "Star note"
                        }
                        aria-pressed={note.isStarred}
                        className={cn(
                          "size-6 transition-opacity",
                          note.isStarred
                            ? "text-amber-500 opacity-100 [&_path]:fill-current"
                            : "opacity-0 group-hover:opacity-100",
                        )}
                        onClick={(e) => {
                          e.stopPropagation();
                          onToggleStar(note.id, !note.isStarred);
                        }}
                      >
                        <HugeiconsIcon
                          icon={StarIcon}
                          size={13}
                          strokeWidth={2}
                        />
                      </Button>
                      <Button
                        type="button"
                        size="icon-sm"
                        variant="ghost"
                        aria-label={note.isPinned ? "Unpin note" : "Pin note"}
                        aria-pressed={note.isPinned}
                        className={cn(
                          "size-6 opacity-0 transition-opacity group-hover:opacity-100",
                          note.isPinned && "text-foreground opacity-100",
                        )}
                        onClick={(e) => {
                          e.stopPropagation();
                          onTogglePin(note.id, !note.isPinned);
                        }}
                      >
                        <HugeiconsIcon
                          icon={PinIcon}
                          size={13}
                          strokeWidth={2}
                        />
                      </Button>
                      <Button
                        type="button"
                        size="icon-sm"
                        variant="ghost"
                        aria-label="Delete note"
                        className="size-6 opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
                        onClick={(e) => {
                          e.stopPropagation();
                          onDeleteNote(note.id);
                        }}
                      >
                        <HugeiconsIcon
                          icon={Delete02Icon}
                          size={13}
                          strokeWidth={2}
                        />
                      </Button>
                    </div>
                  </div>
                  <p className="mt-0.5 line-clamp-2 break-words text-xs text-muted-foreground">
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
