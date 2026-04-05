"use client";

import { Add01Icon, Notebook01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { usePeriodicSync } from "@/hooks/use-periodic-sync";
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
import {
  useCreateNotebookNote,
  useDeleteNotebookNote,
  useDeleteNotebookImage,
  useNotebookNotes,
  useUpdateNotebookNote,
  useUploadNotebookImage,
} from "@/hooks/notebook";
import { useJournalEntriesForAccount } from "@/hooks/journal";
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
  const { data: trades = [] } = useJournalEntriesForAccount(activeAccount?.id ?? null);
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [deletingNoteId, setDeletingNoteId] = useState<string | null>(null);
  const lastSavedByNoteRef = useRef<Record<string, string>>({});
  const latestSaveRequestRef = useRef(0);

  useEffect(() => {
    if (notes.length === 0) {
      setSelectedNoteId(null);
      return;
    }

    const hasSelectedNote = notes.some((note) => note.id === selectedNoteId);
    if (!hasSelectedNote) {
      setSelectedNoteId(notes[0]?.id ?? null);
    }
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

  const handleCreateNote = () => {
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

  const handleFlush = useCallback(
    (noteId: string, change: { documentJson: string; accountId: string }) => {
      const saveRequestId = ++latestSaveRequestRef.current;
      const toastId = `notebook-save-${noteId}`;

      toast.loading("Saving note...", { id: toastId });

      updateNoteMutation.mutate({
        id: noteId,
        input: change,
      }, {
        onSuccess: () => {
          if (latestSaveRequestRef.current !== saveRequestId) return;
          toast.success("Note saved.", { id: toastId, duration: 1500 });
        },
        onError: (error) => {
          if (latestSaveRequestRef.current !== saveRequestId) return;
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to save note."),
            { id: toastId },
          );
        },
      });
    },
    [updateNoteMutation],
  );

  const { enqueue } = usePeriodicSync(handleFlush);

  const handleSerializedChange = (serializedEditorState: string) => {
    if (!selectedNote) return;
    if (lastSavedByNoteRef.current[selectedNote.id] === serializedEditorState) return;

    lastSavedByNoteRef.current[selectedNote.id] = serializedEditorState;

    enqueue(selectedNote.id, {
      documentJson: serializedEditorState,
      accountId: selectedNote.accountId,
    });
  };

  const draftStorageKey = useMemo(
    () =>
      selectedNote
        ? `tradstry-notebook-editor-state:${selectedNote.id}`
        : "tradstry-notebook-editor-state",
    [selectedNote],
  );

  return (
    <section className="mt-10 space-y-4">
      <div className="mx-auto flex w-full max-w-5xl justify-end px-4 sm:px-6 lg:px-10">
        <ManageNotebook
          notes={notes}
          selectedNoteId={selectedNote?.id ?? null}
          activeAccountName={activeAccount?.name ?? null}
          disabled={!activeAccount}
          isCreating={createNoteMutation.isPending}
          deletingNoteId={deletingNoteId}
          onCreateNote={handleCreateNote}
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
          <Empty className="min-h-[42rem] rounded-[2rem] border border-slate-200 bg-white">
            <EmptyHeader>
              <EmptyMedia variant="icon" className="size-12 rounded-xl">
                <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
              </EmptyMedia>
              <EmptyTitle className="text-base font-semibold text-slate-950">
                No active account
              </EmptyTitle>
              <EmptyDescription className="text-sm text-slate-500">
                Select or create an account before opening the notebook editor.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        </div>
      ) : notes.length === 0 ? (
        <div className="mx-auto w-full max-w-5xl px-4 sm:px-6 lg:px-10">
          <Empty className="min-h-[42rem] rounded-[2rem] border border-slate-200 bg-white">
            <EmptyHeader>
              <EmptyMedia variant="icon" className="size-12 rounded-xl">
                <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
              </EmptyMedia>
              <EmptyTitle className="text-base font-semibold text-slate-950">
                No notes yet
              </EmptyTitle>
              <EmptyDescription className="text-sm text-slate-500">
                Create your first note for {activeAccount.name} to start writing
                headers, ideas, and tagged trade context.
              </EmptyDescription>
            </EmptyHeader>
            <EmptyContent>
              <Button
                type="button"
                size="lg"
                onClick={handleCreateNote}
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
              ? async (file) => {
                  const toastId = toast.loading("Uploading image...");

                  try {
                    const image = await uploadImageMutation.mutateAsync({
                      noteId: selectedNote.id,
                      file,
                    });
                    toast.success("Image uploaded.", { id: toastId });
                    return image;
                  } catch (error) {
                    toast.error(
                      getNotebookActionErrorMessage(
                        error,
                        "Failed to upload image.",
                      ),
                      { id: toastId },
                    );
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
          linkedTrades={trades.filter((t) => selectedNote?.tradeIds?.includes(t.id))}
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
