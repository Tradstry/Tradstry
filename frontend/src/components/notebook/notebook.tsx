"use client";

import { Add01Icon, Notebook01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { DEFAULT_NOTE_DOC } from "@tradstry/notebook-core";
import { useCallback, useEffect, useRef, useState } from "react";
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
  useCreateNotebookFolder,
  useCreateNotebookNote,
  useDeleteNotebookFolder,
  useDeleteNotebookImage,
  useDeleteNotebookNote,
  useMoveNotebookNode,
  useNotebookFolders,
  useNotebookNotes,
  useRenameNotebookFolder,
  useSetNotebookNoteFlags,
  useUpdateNotebookNote,
  useUploadNotebookMedia,
} from "@/hooks/notebook";
import type { UploadProgress } from "@/lib/service/notebook";
import { cn } from "@/lib/utils";
import { NotebookEditor } from "./editor";
import { FolderList } from "./folder-list";
import { NoteList } from "./note-list";

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
    refetch: refetchNotes,
  } = useNotebookNotes(activeAccount?.id ?? null);
  const createNoteMutation = useCreateNotebookNote();
  const deleteNoteMutation = useDeleteNotebookNote();
  const uploadMediaMutation = useUploadNotebookMedia();
  const deleteImageMutation = useDeleteNotebookImage();
  const updateNoteMutation = useUpdateNotebookNote();
  const createFolderMutation = useCreateNotebookFolder();
  const renameFolderMutation = useRenameNotebookFolder();
  const deleteFolderMutation = useDeleteNotebookFolder();
  const moveNodeMutation = useMoveNotebookNode();
  const setFlagsMutation = useSetNotebookNoteFlags();
  const { data: folders = [] } = useNotebookFolders(activeAccount?.id ?? null);
  const { data: trades = [] } = useJournalEntriesForAccount(
    activeAccount?.id ?? null,
  );
  const [selectedNoteId, setSelectedNoteId] = useState<string | null>(null);
  const [deletingNoteId, setDeletingNoteId] = useState<string | null>(null);
  // Folder filter: "all" | "uncat" | <folderId>. And the collapsible folder rail.
  const [active, setActive] = useState<string>("all");
  const [collapsed, setCollapsed] = useState(false);

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
  const isNotesLoading = isLoading || isPending;

  const handleCreateNote = (folderId: string | null = null) => {
    if (!activeAccount) {
      return;
    }

    const toastId = toast.loading("Creating note...");

    createNoteMutation.mutate(
      {
        accountId: activeAccount.id,
        documentJson: DEFAULT_NOTE_DOC,
        tradeIds: [],
        folderId,
      },
      {
        onSuccess: (note) => {
          toast.success("Note created.", { id: toastId });
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

  const accountId = activeAccount?.id ?? null;

  const handleCreateFolder = (
    name: string,
    parentFolderId: string | null = null,
  ) => {
    if (!accountId) return;
    createFolderMutation.mutate(
      { accountId, name, parentFolderId },
      {
        onSuccess: (folder) => setActive(folder.id),
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to create folder."),
          ),
      },
    );
  };

  const handleRenameFolder = (id: string, name: string) => {
    renameFolderMutation.mutate(
      { id, name },
      {
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to rename folder."),
          ),
      },
    );
  };

  const handleDeleteFolder = (id: string) => {
    if (!accountId) return;
    deleteFolderMutation.mutate(
      { id, accountId },
      {
        onSuccess: () => setActive((a) => (a === id ? "all" : a)),
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to delete folder."),
          ),
      },
    );
  };

  const handleMoveNote = (noteId: string, folderId: string | null) => {
    if (!accountId) return;
    const note = notes.find((n) => n.id === noteId);
    if (!note || note.folderId === folderId) return;
    moveNodeMutation.mutate(
      {
        accountId,
        nodeId: noteId,
        nodeType: "NOTE",
        newParentFolderId: folderId,
        newSortOrder: 0,
      },
      {
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to move note."),
          ),
      },
    );
  };

  const handleMoveFolder = (
    folderId: string,
    newParentFolderId: string | null,
  ) => {
    if (!accountId || folderId === newParentFolderId) return;
    const folder = folders.find((f) => f.id === folderId);
    if (!folder || folder.parentFolderId === newParentFolderId) return;
    // Guard against dropping a folder into its own descendant, which would orphan the subtree.
    let cursor = newParentFolderId;
    while (cursor) {
      if (cursor === folderId) return;
      cursor = folders.find((f) => f.id === cursor)?.parentFolderId ?? null;
    }
    moveNodeMutation.mutate(
      {
        accountId,
        nodeId: folderId,
        nodeType: "FOLDER",
        newParentFolderId,
        newSortOrder: 0,
      },
      {
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to move folder."),
          ),
      },
    );
  };

  const handleToggleStar = (noteId: string, isStarred: boolean) => {
    setFlagsMutation.mutate(
      { id: noteId, isStarred },
      {
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to star note."),
          ),
      },
    );
  };

  const handleTogglePin = (noteId: string, isPinned: boolean) => {
    setFlagsMutation.mutate(
      { id: noteId, isPinned },
      {
        onError: (error) =>
          toast.error(
            getNotebookActionErrorMessage(error, "Failed to pin note."),
          ),
      },
    );
  };

  // Notes shown in the list, filtered by the active selection, then pinned-first so a
  // pinned note floats to the top of whatever view you're in (stable within each group).
  const filtered = notes
    .filter((n) =>
      active === "all"
        ? true
        : active === "starred"
          ? n.isStarred
          : active === "uncat"
            ? n.folderId == null
            : n.folderId === active,
    )
    .sort((a, b) => Number(b.isPinned) - Number(a.isPinned));
  const listTitle =
    active === "all"
      ? "All notes"
      : active === "starred"
        ? "Favourites"
        : active === "uncat"
          ? "Uncategorized"
          : (folders.find((f) => f.id === active)?.name ?? "Notes");

  // Keep the selected note valid within the active filter; fall back to the first.
  useEffect(() => {
    if (filtered.length === 0) return;
    if (selectedNoteId && filtered.some((n) => n.id === selectedNoteId)) return;
    setSelectedNoteId(filtered[0]?.id ?? null);
  }, [filtered, selectedNoteId]);

  const toggleSidebar = useCallback(() => setCollapsed((c) => !c), []);
  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "\\") {
        e.preventDefault();
        toggleSidebar();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [toggleSidebar]);

  const loading = accountsLoading || (activeAccount && isNotesLoading);

  return (
    <div className="flex h-full min-h-0 w-full">
      <div
        inert={collapsed}
        className={cn(
          "h-full shrink-0 overflow-hidden transition-[width] duration-200 ease-out",
          collapsed ? "w-0" : "w-60",
        )}
      >
        <FolderList
          folders={folders}
          notes={notes}
          active={active}
          onSelect={setActive}
          onCreateFolder={handleCreateFolder}
          onRenameFolder={handleRenameFolder}
          onDeleteFolder={handleDeleteFolder}
          onMoveNote={handleMoveNote}
          onMoveFolder={handleMoveFolder}
        />
      </div>

      <NoteList
        title={listTitle}
        notes={filtered}
        selectedId={selectedNote?.id ?? null}
        onSelect={setSelectedNoteId}
        onCreateNote={() =>
          handleCreateNote(
            active !== "all" && active !== "uncat" && active !== "starred"
              ? active
              : null,
          )
        }
        onDeleteNote={handleDeleteNote}
        onToggleStar={handleToggleStar}
        onTogglePin={handleTogglePin}
        sidebarCollapsed={collapsed}
        onToggleSidebar={toggleSidebar}
      />

      <div className="flex min-h-0 flex-1 flex-col">{renderEditorPane()}</div>
    </div>
  );

  function renderEditorPane() {
    if (loading) {
      return (
        <div className="flex-1 p-6">
          <Skeleton
            data-testid="notebook-notes-loading"
            className="h-full rounded-2xl"
          />
        </div>
      );
    }

    if (!activeAccount) {
      return (
        <Empty className="flex-1">
          <EmptyHeader>
            <EmptyMedia variant="icon" className="size-12 rounded-xl">
              <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
            </EmptyMedia>
            <EmptyTitle>No active account</EmptyTitle>
            <EmptyDescription>
              Select or create an account before opening the notebook editor.
            </EmptyDescription>
          </EmptyHeader>
        </Empty>
      );
    }

    if (!selectedNote) {
      return (
        <Empty className="flex-1">
          <EmptyHeader>
            <EmptyMedia variant="icon" className="size-12 rounded-xl">
              <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
            </EmptyMedia>
            <EmptyTitle>
              {notes.length === 0 ? "No notes yet" : "No note selected"}
            </EmptyTitle>
            <EmptyDescription>Create a note to start writing.</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button
              onClick={() =>
                handleCreateNote(
                  active !== "all" && active !== "uncat" ? active : null,
                )
              }
              disabled={createNoteMutation.isPending}
            >
              <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
              {createNoteMutation.isPending ? "Creating..." : "New note"}
            </Button>
          </EmptyContent>
        </Empty>
      );
    }

    return renderEditor(selectedNote);
  }

  function renderEditor(note: (typeof notes)[number]) {
    return (
      <NotebookEditor
        key={note.id}
        noteId={note.id}
        images={note.images}
        onNeedMediaRefresh={() => {
          void refetchNotes();
        }}
        onUploadMedia={async (file, hash, signal) => {
          const label = file.type.startsWith("video/") ? "video" : "image";
          const toastId = toast.loading(`Uploading ${label}…`);
          try {
            const image = await uploadMediaMutation.mutateAsync({
              noteId: note.id,
              hash,
              file,
              signal,
              onProgress: (progress) => {
                toast.loading(
                  <UploadProgressToast label={label} progress={progress} />,
                  { id: toastId },
                );
              },
            });
            toast.success(
              `${label === "video" ? "Video" : "Image"} uploaded.`,
              {
                id: toastId,
              },
            );
            return image;
          } catch (error) {
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
        }}
        onDeleteImage={async (hash) => {
          const toastId = toast.loading("Deleting image...");
          try {
            await deleteImageMutation.mutateAsync({ hash, noteId: note.id });
            toast.success("Image deleted.", { id: toastId });
          } catch (error) {
            toast.error(
              getNotebookActionErrorMessage(error, "Failed to delete image."),
              { id: toastId },
            );
            throw error;
          }
        }}
        trades={trades}
        onLinkTrade={(tradeId) => {
          const currentIds = note.tradeIds ?? [];
          if (currentIds.includes(tradeId)) return;
          updateNoteMutation.mutate({
            id: note.id,
            input: {
              tradeIds: [...currentIds, tradeId],
              accountId: note.accountId,
            },
          });
        }}
        onUnlinkTrade={(tradeId) => {
          const currentIds = note.tradeIds ?? [];
          updateNoteMutation.mutate({
            id: note.id,
            input: {
              tradeIds: currentIds.filter((id) => id !== tradeId),
              accountId: note.accountId,
            },
          });
        }}
      />
    );
  }
}
