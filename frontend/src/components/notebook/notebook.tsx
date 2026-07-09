"use client";

import { Add01Icon, Notebook01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import {
  useAccountsLoading,
  useActiveAccount,
} from "@/components/accounts/hooks";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { Skeleton } from "@/components/ui/skeleton";
import { useJournalEntriesForAccount } from "@/hooks/journal";
import {
  useCreateNotebookNote,
  useDeleteNotebookImage,
  useDeleteNotebookNote,
  useNotebookNotes,
  useUpdateNotebookNote,
  useUploadNotebookImage,
} from "@/hooks/notebook";
import { useNotebookPanelStore } from "@/hooks/notebook-panel";
import { usePeriodicSync } from "@/hooks/use-periodic-sync";
import type { UploadProgress } from "@/lib/service/notebook";
import {
  createDefaultNotebookDocumentJson,
  mergeNotebookImagesIntoDocumentJson,
  NotebookEditor,
  normalizeNotebookDocumentJson,
} from "./editor";
import { ManageNotebook } from "./manage-notebook";

function getNotebookActionErrorMessage(
  error: unknown,
  fallback: string,
): string {
  if (error instanceof Error && error.message.trim().length > 0) {
    return error.message;
  }

  return fallback;
}

function formatMb(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}

function UploadProgressToast({
  label,
  progress,
}: {
  label: string;
  progress: UploadProgress;
}) {
  return (
    <div className="flex w-56 flex-col gap-1.5">
      <span className="text-sm font-medium text-foreground">
        Uploading {label}… {progress.percent}%
      </span>
      <div className="h-1.5 w-full overflow-hidden rounded-full bg-muted">
        <div
          className="h-full rounded-full bg-foreground transition-[width] duration-150"
          style={{ width: `${progress.percent}%` }}
        />
      </div>
      <span className="text-xs text-muted-foreground">
        {formatMb(progress.loaded)} / {formatMb(progress.total)} MB
      </span>
    </div>
  );
}

export function Notebook() {
  const accountsLoading = useAccountsLoading();
  const activeAccount = useActiveAccount();
  const {
    data: notes = [],
    isLoading,
    isPending,
  } = useNotebookNotes(activeAccount?.id ?? null);
  const createNoteMutation = useCreateNotebookNote();
  const deleteNoteMutation = useDeleteNotebookNote();
  const uploadImageMutation = useUploadNotebookImage();
  const deleteImageMutation = useDeleteNotebookImage();
  const updateNoteMutation = useUpdateNotebookNote();
  const { data: trades = [] } = useJournalEntriesForAccount(
    activeAccount?.id ?? null,
  );
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [deletingNoteId, setDeletingNoteId] = useState<string | null>(null);
  const lastSavedByNoteRef = useRef<Record<string, string>>({});
  const latestSaveRequestRef = useRef(0);

  // `?note=<id>` deep link, e.g. from a principle's evidence note. Read from
  // `window.location` rather than `useSearchParams` so this prerendered route
  // does not need a Suspense boundary.
  const deepLinkedNoteIdRef = useRef<string | null>(
    typeof window === "undefined"
      ? null
      : new URLSearchParams(window.location.search).get("note"),
  );

  useEffect(() => {
    if (notes.length === 0) {
      setSelectedNoteId(null);
      return;
    }

    const hasSelectedNote = notes.some((note) => note.id === selectedNoteId);
    if (hasSelectedNote) return;

    // Honor the deep link once, before falling back to the first note. It is
    // consumed either way, so deleting a note never bounces the user back.
    const deepLinkedId = deepLinkedNoteIdRef.current;
    deepLinkedNoteIdRef.current = null;
    const deepLinked =
      deepLinkedId && notes.some((note) => note.id === deepLinkedId)
        ? deepLinkedId
        : null;

    setSelectedNoteId(deepLinked ?? notes[0]?.id ?? null);
  }, [notes, selectedNoteId]);

  const selectedNote =
    notes.find((note) => note.id === selectedNoteId) ?? notes[0] ?? null;
  const selectedNoteDocumentJson = useMemo(
    () =>
      mergeNotebookImagesIntoDocumentJson(
        selectedNote?.documentJson ?? null,
        selectedNote?.images ?? [],
      ),
    [selectedNote],
  );

  const isNotesLoading = isLoading || isPending;

  const handleCreateNote = (folderId: string | null = null) => {
    if (!activeAccount) {
      return;
    }

    const documentJson = createDefaultNotebookDocumentJson();
    const toastId = toast.loading("Creating note...");

    createNoteMutation.mutate(
      {
        accountId: activeAccount.id,
        documentJson,
        tradeIds: [],
        folderId,
      },
      {
        onSuccess: (note) => {
          toast.success("Note created.", { id: toastId });
          lastSavedByNoteRef.current[note.id] = note.documentJson;
          setSelectedNoteId(note.id);
        },
        onError: (error) => {
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to create note."),
            { id: toastId },
          );
        },
      },
    );
  };

  const handleDeleteNote = (noteId: string) => {
    const toastId = toast.loading("Deleting note...");
    setDeletingNoteId(noteId);

    deleteNoteMutation.mutate(noteId, {
      onSuccess: () => {
        toast.success("Note deleted.", { id: toastId });
        setSelectedNoteId((currentSelectedNoteId) => {
          if (currentSelectedNoteId !== noteId) {
            return currentSelectedNoteId;
          }

          return notes.find((note) => note.id !== noteId)?.id ?? null;
        });
      },
      onError: (error) => {
        toast.error(
          getNotebookActionErrorMessage(error, "Failed to delete note."),
          { id: toastId },
        );
      },
      onSettled: () => {
        setDeletingNoteId((currentDeletingNoteId) =>
          currentDeletingNoteId === noteId ? null : currentDeletingNoteId,
        );
      },
    });
  };

  useEffect(() => {
    if (!selectedNote) {
      return;
    }

    const normalizedDocumentJson = normalizeNotebookDocumentJson(
      selectedNoteDocumentJson,
    );

    if (!normalizedDocumentJson) {
      return;
    }

    lastSavedByNoteRef.current[selectedNote.id] = normalizedDocumentJson;
  }, [selectedNote, selectedNoteDocumentJson]);

  const lastFlushAtRef = useRef<Record<string, number>>({});
  const lastFlushedContentRef = useRef<Record<string, string>>({});

  const handleFlush = useCallback(
    (noteId: string, change: { documentJson: string; accountId: string }) => {
      // Guard 1: skip if content is byte-identical to what we last flushed
      // for this note. Defends against any path that re-enqueues the same
      // serialized state (Lexical sometimes fires onChange for selection-
      // only mutations even with ignoreSelectionChange).
      if (lastFlushedContentRef.current[noteId] === change.documentJson) {
        return;
      }

      // Guard 2: throttle to one save per note per 2s. The periodic-sync
      // hook should already gate this to the 5-minute interval, but if any
      // upstream path ever fires flush() faster than that we want a hard
      // cap so the user doesn't see save spam.
      const now = Date.now();
      const last = lastFlushAtRef.current[noteId] ?? 0;
      if (now - last < 2000) {
        return;
      }
      lastFlushAtRef.current[noteId] = now;
      lastFlushedContentRef.current[noteId] = change.documentJson;

      const saveRequestId = ++latestSaveRequestRef.current;

      // Auto-saves run silently — no "Saving..." or "Saved" toast. Users
      // don't need to see every 5-minute background write, and the noise
      // makes the editor feel busy. Only errors are surfaced.
      updateNoteMutation.mutate(
        {
          id: noteId,
          input: change,
        },
        {
          onError: (error) => {
            if (latestSaveRequestRef.current !== saveRequestId) return;
            toast.error(
              getNotebookActionErrorMessage(error, "Failed to save note."),
              { id: `notebook-save-${noteId}` },
            );
          },
        },
      );
    },
    [updateNoteMutation],
  );

  const { enqueue } = usePeriodicSync(handleFlush);

  const handleSerializedChange = (serializedEditorState: string) => {
    if (!selectedNote) return;
    if (lastSavedByNoteRef.current[selectedNote.id] === serializedEditorState)
      return;

    lastSavedByNoteRef.current[selectedNote.id] = serializedEditorState;

    enqueue(selectedNote.id, {
      documentJson: serializedEditorState,
      accountId: selectedNote.accountId,
    });
  };

  const notesPanelOpen = useNotebookPanelStore((s) => s.isOpen);

  const draftStorageKey = useMemo(
    () =>
      selectedNote
        ? `tradstry-notebook-editor-state:${selectedNote.id}`
        : "tradstry-notebook-editor-state",
    [selectedNote],
  );

  return (
    <section
      className="mt-10 space-y-4 transition-[margin] duration-200 ease-in-out"
      style={{ marginRight: notesPanelOpen ? 380 : 0 }}
    >
      <div className="mx-auto flex w-full max-w-5xl justify-end px-4 sm:px-6 lg:px-10">
        <ManageNotebook
          accountId={activeAccount?.id ?? null}
          notes={notes}
          selectedNoteId={selectedNote?.id ?? null}
          activeAccountName={activeAccount?.name ?? null}
          trades={trades}
          disabled={!activeAccount}
          isCreating={createNoteMutation.isPending}
          deletingNoteId={deletingNoteId}
          onCreateNote={() => handleCreateNote(null)}
          onCreateNoteInFolder={(folderId) => handleCreateNote(folderId)}
          onSelectNote={setSelectedNoteId}
          onDeleteNote={handleDeleteNote}
        />
      </div>
      {accountsLoading || (activeAccount && isNotesLoading) ? (
        <div className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
          <Skeleton className="h-[42rem] rounded-[2rem]" />
        </div>
      ) : !activeAccount ? (
        <div className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
          <Empty className="min-h-[42rem] rounded-[2rem] border border-border bg-popover">
            <EmptyHeader>
              <EmptyMedia variant="icon" className="size-12 rounded-xl">
                <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
              </EmptyMedia>
              <EmptyTitle className="text-base font-semibold text-foreground">
                No active account
              </EmptyTitle>
              <EmptyDescription className="text-sm text-muted-foreground">
                Select or create an account before opening the notebook editor.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        </div>
      ) : notes.length === 0 ? (
        <div className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
          <Empty className="min-h-[42rem] rounded-[2rem] border border-border bg-popover">
            <EmptyHeader>
              <EmptyMedia variant="icon" className="size-12 rounded-xl">
                <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
              </EmptyMedia>
              <EmptyTitle className="text-base font-semibold text-foreground">
                No notes yet
              </EmptyTitle>
              <EmptyDescription className="text-sm text-muted-foreground">
                Create your first note for {activeAccount.name} to start writing
                headers, ideas, and tagged trade context.
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button
                type="button"
                size="lg"
                onClick={() => handleCreateNote(null)}
                disabled={createNoteMutation.isPending}
              >
                <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
                {createNoteMutation.isPending ? "Creating..." : "Create Note"}
              </Button>
            </EmptyContent>
          </Empty>
        </div>
      ) : (
        <NotebookEditor
          key={selectedNote?.id ?? "notebook-editor"}
          initialDocumentJson={selectedNoteDocumentJson}
          draftStorageKey={draftStorageKey}
          onSerializedChange={handleSerializedChange}
          onUploadImage={
            selectedNote
              ? async (file, signal) => {
                  const label = file.type.startsWith("video/")
                    ? "video"
                    : "image";
                  const toastId = toast.loading(`Uploading ${label}…`);

                  try {
                    const image = await uploadImageMutation.mutateAsync({
                      noteId: selectedNote.id,
                      file,
                      signal,
                      onProgress: (progress) => {
                        toast.loading(
                          <UploadProgressToast
                            label={label}
                            progress={progress}
                          />,
                          { id: toastId },
                        );
                      },
                    });
                    toast.success(
                      `${label === "video" ? "Video" : "Image"} uploaded.`,
                      { id: toastId },
                    );
                    return image;
                  } catch (error) {
                    // User-initiated cancel: don't surface it as a failure.
                    if (signal?.aborted) {
                      toast.dismiss(toastId);
                    } else {
                      toast.error(
                        getNotebookActionErrorMessage(
                          error,
                          `Failed to upload ${label}.`,
                        ),
                        { id: toastId },
                      );
                    }
                    throw error;
                  }
                }
              : undefined
          }
          onDeleteImage={
            selectedNote
              ? async (imageId) => {
                  const toastId = toast.loading("Deleting image...");

                  try {
                    await deleteImageMutation.mutateAsync(imageId);
                    toast.success("Image deleted.", { id: toastId });
                  } catch (error) {
                    toast.error(
                      getNotebookActionErrorMessage(
                        error,
                        "Failed to delete image.",
                      ),
                      { id: toastId },
                    );
                    throw error;
                  }
                }
              : undefined
          }
          trades={trades}
          onLinkTrade={
            selectedNote
              ? (tradeId) => {
                  const currentIds = selectedNote.tradeIds ?? [];
                  if (currentIds.includes(tradeId)) return;
                  updateNoteMutation.mutate({
                    id: selectedNote.id,
                    input: {
                      tradeIds: [...currentIds, tradeId],
                      accountId: selectedNote.accountId,
                    },
                  });
                }
              : undefined
          }
          onUnlinkTrade={
            selectedNote
              ? (tradeId) => {
                  const currentIds = selectedNote.tradeIds ?? [];
                  updateNoteMutation.mutate({
                    id: selectedNote.id,
                    input: {
                      tradeIds: currentIds.filter((id) => id !== tradeId),
                      accountId: selectedNote.accountId,
                    },
                  });
                }
              : undefined
          }
        />
      )}
    </section>
  );
}
