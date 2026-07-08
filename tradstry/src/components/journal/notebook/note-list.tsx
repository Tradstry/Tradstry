import { PlusIcon, TrashIcon } from "@phosphor-icons/react";
import { ScrollArea } from "../../user-interface";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { notePreview, type Note } from "./types";

function relTime(iso: string): string {
  const d = new Date(iso);
  const diff = Date.now() - d.getTime();
  const min = Math.floor(diff / 60000);
  if (min < 1) return "just now";
  if (min < 60) return `${min}m ago`;
  const h = Math.floor(min / 60);
  if (h < 24) return `${h}h ago`;
  const days = Math.floor(h / 24);
  if (days < 7) return `${days}d ago`;
  return d.toLocaleDateString("en-US", { month: "short", day: "numeric" });
}

export function NoteList({
  title,
  notes,
  selectedId,
  onSelect,
  onCreateNote,
  onDeleteNote,
}: {
  title: string;
  notes: Note[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  onCreateNote: () => void;
  onDeleteNote: (id: string) => void;
}) {
  return (
    <div className="flex w-72 shrink-0 flex-col border-r border-zinc-200/70 dark:border-zinc-800/70">
      <div className="flex items-center justify-between gap-2 px-3 py-2.5">
        <span className="truncate text-sm font-semibold">{title}</span>
        <Button
          type="button"
          size="icon-sm"
          onClick={onCreateNote}
          aria-label="New note"
        >
          <PlusIcon size={15} />
        </Button>
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
                  <p className="mt-1 text-[11px] text-muted-foreground/70">
                    {relTime(note.updatedAt)}
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
