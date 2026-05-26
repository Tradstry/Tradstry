"use client";

import {
  Add01Icon,
  Cancel01Icon,
  Delete02Icon,
  Note01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useNotebookPanelStore } from "@/hooks/notebook-panel";
import type { JournalEntry } from "@/lib/types/journal";
import type { NotebookNote } from "@/lib/types/notebook";
import { cn } from "@/lib/utils";

export function ManageNotebook({
  notes,
  selectedNoteId,
  activeAccountName,
  trades = [],
  disabled = false,
  isCreating = false,
  deletingNoteId = null,
  onCreateNote,
  onSelectNote,
  onDeleteNote,
}: {
  notes: NotebookNote[];
  selectedNoteId: string | null;
  activeAccountName: string | null;
  trades?: JournalEntry[];
  disabled?: boolean;
  isCreating?: boolean;
  deletingNoteId?: string | null;
  onCreateNote: () => void;
  onSelectNote: (noteId: string) => void;
  onDeleteNote: (noteId: string) => void;
}) {
  const [confirmingNoteId, setConfirmingNoteId] = useState<string | null>(null);
  const isOpen = useNotebookPanelStore((s) => s.isOpen);
  const setOpen = useNotebookPanelStore((s) => s.setOpen);
  const toggleOpen = useNotebookPanelStore((s) => s.toggleOpen);

  return (
    <>
      <Button
        type="button"
        variant="outline"
        size="lg"
        disabled={disabled}
        onClick={toggleOpen}
      >
        Manage Notes
      </Button>
      {isOpen && (
        <div className="fixed top-0 right-0 z-40 flex h-full w-[380px] flex-col border-l bg-background shadow-lg animate-in slide-in-from-right duration-200">
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="absolute top-4 left-4 z-10"
            onClick={() => setOpen(false)}
          >
            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
            <span className="sr-only">Close</span>
          </Button>
          <div className="border-b border-slate-200 px-6 py-5 pl-14">
            <h2 className="text-base font-semibold text-slate-950">
              Manage Notes
            </h2>
            <p className="text-sm text-slate-500">
              {activeAccountName
                ? `Organize notes for ${activeAccountName}.`
                : "Select an account to manage notes."}
            </p>
          </div>
          <div className="border-b border-slate-200 px-6 py-4">
          <Button
            type="button"
            className="w-full justify-center"
            onClick={onCreateNote}
            disabled={disabled || isCreating}
          >
            <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
            {isCreating ? "Creating..." : "Create Note"}
          </Button>
        </div>
        <ScrollArea className="flex-1">
          <div className="px-3 py-3">
            {notes.length === 0 ? (
              <div className="rounded-xl border border-dashed border-slate-200 px-4 py-6 text-center text-sm leading-6 text-slate-500">
                No notes yet.
              </div>
            ) : (
              <div className="space-y-2">
                {notes.map((note) => (
                  <div
                    key={note.id}
                    className={cn(
                      "flex items-start gap-2 rounded-xl border px-4 py-3 transition-colors",
                      selectedNoteId === note.id
                        ? "border-slate-900 bg-slate-900 text-white"
                        : "border-slate-200 bg-white text-slate-900 hover:bg-slate-50",
                    )}
                  >
                    <button
                      type="button"
                      className="flex min-w-0 flex-1 items-start gap-3 text-left"
                      onClick={() => onSelectNote(note.id)}
                    >
                      <span
                        className={cn(
                          "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg",
                          selectedNoteId === note.id
                            ? "bg-white/10 text-white"
                            : "bg-slate-100 text-slate-700",
                        )}
                      >
                        <HugeiconsIcon icon={Note01Icon} strokeWidth={2} />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm font-medium">
                          {note.title}
                        </span>
                        <span
                          className={cn(
                            "mt-1 block text-xs",
                            selectedNoteId === note.id
                              ? "text-slate-300"
                              : "text-slate-500",
                          )}
                        >
                          {new Date(note.updatedAt).toLocaleString()}
                        </span>
                        {note.tradeIds.length > 0 && (() => {
                          const linked = trades.filter((t) => note.tradeIds.includes(t.id));
                          if (linked.length === 0) return null;
                          return (
                            <span className="mt-1.5 flex flex-wrap gap-1">
                              {linked.map((t) => (
                                <span
                                  key={t.id}
                                  className={cn(
                                    "inline-flex items-center gap-1 rounded-full px-1.5 py-0 text-[0.6rem] font-medium",
                                    selectedNoteId === note.id
                                      ? "bg-white/10 text-slate-200"
                                      : t.status === "profit"
                                        ? "bg-emerald-50 text-emerald-700"
                                        : "bg-rose-50 text-rose-700",
                                  )}
                                >
                                  {t.symbol}
                                  <span className="opacity-60">
                                    {t.tradeType === "long" ? "L" : "S"}
                                  </span>
                                </span>
                              ))}
                            </span>
                          );
                        })()}
                      </span>
                    </button>

                    <Popover
                      open={confirmingNoteId === note.id}
                      onOpenChange={(open) => {
                        setConfirmingNoteId(open ? note.id : null);
                      }}
                    >
                      <PopoverTrigger asChild>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-sm"
                          className={cn(
                            "mt-0.5 shrink-0 rounded-lg",
                            selectedNoteId === note.id
                              ? "text-slate-300 hover:bg-white/10 hover:text-white"
                              : "text-slate-500 hover:bg-slate-100 hover:text-slate-950",
                          )}
                          aria-label="Delete note"
                          disabled={deletingNoteId === note.id}
                        >
                          <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                        </Button>
                      </PopoverTrigger>
                      <PopoverContent
                        align="end"
                        className="w-72 space-y-3"
                        onClick={(event) => event.stopPropagation()}
                      >
                        <div className="space-y-1">
                          <p className="text-sm font-semibold text-slate-950">
                            Delete note?
                          </p>
                          <p className="text-sm leading-6 text-slate-500">
                            This permanently deletes this note and its notebook
                            images.
                          </p>
                        </div>
                        <div className="flex justify-end gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => setConfirmingNoteId(null)}
                          >
                            Cancel
                          </Button>
                          <Button
                            type="button"
                            variant="destructive"
                            size="sm"
                            disabled={deletingNoteId === note.id}
                            onClick={() => {
                              onDeleteNote(note.id);
                              setConfirmingNoteId(null);
                            }}
                          >
                            {deletingNoteId === note.id
                              ? "Deleting..."
                              : "Delete"}
                          </Button>
                        </div>
                      </PopoverContent>
                    </Popover>
                  </div>
                ))}
              </div>
            )}
          </div>
        </ScrollArea>
        </div>
      )}
    </>
  );
}
