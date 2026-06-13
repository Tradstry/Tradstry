"use client";

import { Cancel01Icon, FolderAddIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useCreateNotebookFolder, useNotebookTree } from "@/hooks/notebook";
import { useNotebookPanelStore } from "@/hooks/notebook-panel";
import type { JournalEntry } from "@/lib/types/journal";
import type { NotebookNote } from "@/lib/types/notebook";
import { NotebookTree } from "./notebook-tree";

// Notion-style dark tooltip (matches the one used in notebook-tree.tsx).
const TOOLTIP_DARK =
  "border-0 bg-[#2d2d2d] text-white [&_svg]:bg-[#2d2d2d] [&_svg]:fill-[#2d2d2d]";

// Inline "compose" glyph (square + pencil) for the bottom create-note button.
function ComposeIcon() {
  return (
    <svg
      width="20"
      height="20"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
    >
      <path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7" />
      <path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z" />
    </svg>
  );
}

export function ManageNotebook({
  accountId,
  // `notes` is retained for type compatibility with the parent but the tree
  // now sources notes (and folders) directly from `useNotebookTree`.
  notes: _notes,
  selectedNoteId,
  activeAccountName,
  trades = [],
  disabled = false,
  isCreating = false,
  deletingNoteId = null,
  onCreateNote,
  onCreateNoteInFolder,
  onSelectNote,
  onDeleteNote,
}: {
  accountId: string | null;
  notes: NotebookNote[];
  selectedNoteId: string | null;
  activeAccountName: string | null;
  trades?: JournalEntry[];
  disabled?: boolean;
  isCreating?: boolean;
  deletingNoteId?: string | null;
  onCreateNote: () => void;
  onCreateNoteInFolder: (folderId: string | null) => void;
  onSelectNote: (noteId: string) => void;
  onDeleteNote: (noteId: string) => void;
}) {
  const isOpen = useNotebookPanelStore((s) => s.isOpen);
  const setOpen = useNotebookPanelStore((s) => s.setOpen);
  const toggleOpen = useNotebookPanelStore((s) => s.toggleOpen);

  const { tree } = useNotebookTree(accountId);
  const createFolder = useCreateNotebookFolder();

  const handleCreateFolder = (parentFolderId: string | null) => {
    if (!accountId) {
      return;
    }
    const toastId = toast.loading("Creating folder...");
    createFolder.mutate(
      {
        accountId,
        parentFolderId,
        name: "New folder",
      },
      {
        onSuccess: () => {
          toast.success("Folder created.", { id: toastId });
        },
        onError: (error) => {
          toast.error(
            error instanceof Error && error.message.trim().length > 0
              ? error.message
              : "Failed to create folder.",
            { id: toastId },
          );
        },
      },
    );
  };

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
            className="absolute top-4 right-4 z-10"
            onClick={() => setOpen(false)}
          >
            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
            <span className="sr-only">Close</span>
          </Button>
          <div className="border-b border-border py-5 pl-6 pr-12">
            <h2 className="text-base font-semibold text-foreground">
              Manage Notes
            </h2>
            <p className="text-sm text-muted-foreground">
              {activeAccountName
                ? `Organize notes for ${activeAccountName}.`
                : "Select an account to manage notes."}
            </p>
          </div>
          <div className="border-b border-border px-6 py-4">
            <Button
              type="button"
              variant="outline"
              className="w-full justify-center"
              onClick={() => handleCreateFolder(null)}
              disabled={disabled || createFolder.isPending}
            >
              <HugeiconsIcon icon={FolderAddIcon} strokeWidth={2} />
              New Folder
            </Button>
          </div>
          <ScrollArea className="flex-1">
            <div className="px-3 pt-3 pb-24">
              <NotebookTree
                accountId={accountId}
                nodes={tree}
                selectedNoteId={selectedNoteId}
                trades={trades}
                deletingNoteId={deletingNoteId}
                onSelectNote={onSelectNote}
                onDeleteNote={onDeleteNote}
                onCreateNoteInFolder={onCreateNoteInFolder}
                onCreateFolder={handleCreateFolder}
              />
            </div>
          </ScrollArea>

          <Tooltip>
            <TooltipTrigger asChild>
              <Button
                type="button"
                variant="outline"
                size="icon"
                className="absolute right-6 bottom-6 z-10 size-12 rounded-full bg-popover text-foreground shadow-md hover:bg-accent hover:text-foreground"
                aria-label="New note"
                onClick={onCreateNote}
                disabled={disabled || isCreating}
              >
                <ComposeIcon />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="top" className={TOOLTIP_DARK}>
              New…
            </TooltipContent>
          </Tooltip>
        </div>
      )}
    </>
  );
}
