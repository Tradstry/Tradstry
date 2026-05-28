"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useMemo } from "react";
import { useGraphQL } from "@/lib/client";
import * as notebookService from "@/lib/service/notebook";
import type {
  CreateNotebookFolderInput,
  CreateNotebookNoteInput,
  MoveNotebookNodeInput,
  NotebookFolder,
  NotebookNote,
  UpdateNotebookNoteInput,
} from "@/lib/types/notebook";

const NOTEBOOK_KEY = ["notebook"] as const;

const notesKey = (accountId?: string | null) =>
  [...NOTEBOOK_KEY, "notes", accountId ?? null] as const;

const foldersKey = (accountId?: string | null) =>
  [...NOTEBOOK_KEY, "folders", accountId ?? null] as const;

export function useNotebookNotes(accountId?: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<NotebookNote[]>({
    queryKey: notesKey(accountId),
    queryFn: () => notebookService.fetchNotebookNotes(fetcher, accountId),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useNotebookFolders(accountId?: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<NotebookFolder[]>({
    queryKey: foldersKey(accountId),
    queryFn: () => {
      if (!accountId) {
        throw new Error("notebook folders accountId is required");
      }

      return notebookService.fetchNotebookFolders(fetcher, accountId);
    },
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useNotebookNote(id: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<NotebookNote | null>({
    queryKey: [...NOTEBOOK_KEY, "note", id],
    queryFn: () => {
      if (!id) {
        throw new Error("notebook note id is required");
      }

      return notebookService.fetchNotebookNote(fetcher, id);
    },
    enabled: isLoaded && isSignedIn && !!id,
  });
}

export function useCreateNotebookNote() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateNotebookNoteInput) =>
      notebookService.createNotebookNote(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: NOTEBOOK_KEY });
    },
  });
}

export function useUpdateNotebookNote() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      id,
      input,
    }: {
      id: string;
      input: UpdateNotebookNoteInput;
    }) => notebookService.updateNotebookNote(fetcher, id, input),
    onSuccess: (saved) => {
      // Write the server response directly into the cache instead of
      // invalidating + refetching. Invalidation would re-trigger
      // `useNotebookNotes`, which rebuilds the notes array, which forces
      // `selectedNote` and `selectedNoteDocumentJson` to new references,
      // which previously caused the editor to re-hydrate from props mid-
      // typing and lose recent keystrokes.
      queryClient.setQueryData<NotebookNote | null>(
        [...NOTEBOOK_KEY, "note", saved.id],
        saved,
      );
      queryClient.setQueryData<NotebookNote[] | undefined>(
        [...NOTEBOOK_KEY, "notes", saved.accountId ?? null],
        (existing) => {
          if (!existing) return existing;
          let found = false;
          const next = existing.map((n) => {
            if (n.id === saved.id) {
              found = true;
              return saved;
            }
            return n;
          });
          // If the note isn't yet in the cached list (e.g. just-created),
          // append it so the list view still sees the latest write.
          return found ? next : [...next, saved];
        },
      );
    },
  });
}

export function useDeleteNotebookNote() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => notebookService.deleteNotebookNote(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: NOTEBOOK_KEY });
    },
  });
}

export function useCreateNotebookFolder() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateNotebookFolderInput) =>
      notebookService.createNotebookFolder(fetcher, input),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: foldersKey(variables.accountId),
      });
      queryClient.invalidateQueries({
        queryKey: notesKey(variables.accountId),
      });
    },
  });
}

export function useRenameNotebookFolder() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      notebookService.renameNotebookFolder(fetcher, id, name),
    onSuccess: (saved) => {
      queryClient.invalidateQueries({ queryKey: foldersKey(saved.accountId) });
      queryClient.invalidateQueries({ queryKey: notesKey(saved.accountId) });
    },
  });
}

export function useDeleteNotebookFolder() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id }: { id: string; accountId: string }) =>
      notebookService.deleteNotebookFolder(fetcher, id),
    onSuccess: (_data, variables) => {
      queryClient.invalidateQueries({
        queryKey: foldersKey(variables.accountId),
      });
      queryClient.invalidateQueries({
        queryKey: notesKey(variables.accountId),
      });
    },
  });
}

export function useMoveNotebookNode() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: MoveNotebookNodeInput) =>
      notebookService.moveNotebookNode(fetcher, input),
    // Optimistic move mirroring useUpdateNotebookNote's local-cache write: we
    // apply the new parent + sort order immediately so the tree re-renders
    // without waiting for the server round-trip, then roll back on error.
    onMutate: async (input: MoveNotebookNodeInput) => {
      const fKey = foldersKey(input.accountId);
      const nKey = notesKey(input.accountId);

      await queryClient.cancelQueries({ queryKey: fKey });
      await queryClient.cancelQueries({ queryKey: nKey });

      const previousFolders = queryClient.getQueryData<NotebookFolder[]>(fKey);
      const previousNotes = queryClient.getQueryData<NotebookNote[]>(nKey);

      if (input.nodeType === "FOLDER") {
        queryClient.setQueryData<NotebookFolder[] | undefined>(
          fKey,
          (existing) =>
            existing?.map((f) =>
              f.id === input.nodeId
                ? {
                    ...f,
                    parentFolderId: input.newParentFolderId,
                    sortOrder: input.newSortOrder,
                  }
                : f,
            ),
        );
      } else {
        queryClient.setQueryData<NotebookNote[] | undefined>(nKey, (existing) =>
          existing?.map((n) =>
            n.id === input.nodeId
              ? {
                  ...n,
                  folderId: input.newParentFolderId,
                  sortOrder: input.newSortOrder,
                }
              : n,
          ),
        );
      }

      return { previousFolders, previousNotes, fKey, nKey };
    },
    onError: (_err, _input, context) => {
      if (!context) return;
      if (context.previousFolders !== undefined) {
        queryClient.setQueryData(context.fKey, context.previousFolders);
      }
      if (context.previousNotes !== undefined) {
        queryClient.setQueryData(context.nKey, context.previousNotes);
      }
    },
    onSettled: (_data, _err, input) => {
      queryClient.invalidateQueries({ queryKey: foldersKey(input.accountId) });
      queryClient.invalidateQueries({ queryKey: notesKey(input.accountId) });
    },
  });
}

export interface NotebookTreeNode {
  type: "FOLDER" | "NOTE";
  id: string;
  name: string;
  parentId: string | null;
  sortOrder: number;
  children: NotebookTreeNode[];
  note?: NotebookNote;
}

export function useNotebookTree(accountId?: string | null) {
  const foldersQuery = useNotebookFolders(accountId);
  const notesQuery = useNotebookNotes(accountId);

  const folders = useMemo(() => foldersQuery.data ?? [], [foldersQuery.data]);
  const notes = useMemo(() => notesQuery.data ?? [], [notesQuery.data]);

  const tree = useMemo<NotebookTreeNode[]>(() => {
    const flat: NotebookTreeNode[] = [];

    for (const folder of folders) {
      flat.push({
        type: "FOLDER",
        id: folder.id,
        name: folder.name,
        parentId: folder.parentFolderId,
        sortOrder: folder.sortOrder,
        children: [],
      });
    }

    for (const note of notes) {
      flat.push({
        type: "NOTE",
        id: note.id,
        name: note.title,
        parentId: note.folderId,
        sortOrder: note.sortOrder,
        children: [],
        note,
      });
    }

    const byParent = new Map<string | null, NotebookTreeNode[]>();
    for (const node of flat) {
      const bucket = byParent.get(node.parentId);
      if (bucket) {
        bucket.push(node);
      } else {
        byParent.set(node.parentId, [node]);
      }
    }

    for (const bucket of byParent.values()) {
      bucket.sort((a, b) => {
        if (a.sortOrder !== b.sortOrder) {
          return a.sortOrder - b.sortOrder;
        }
        return a.name.localeCompare(b.name);
      });
    }

    const attach = (parentId: string | null): NotebookTreeNode[] => {
      const bucket = byParent.get(parentId) ?? [];
      return bucket.map((node) => ({
        ...node,
        children: node.type === "FOLDER" ? attach(node.id) : [],
      }));
    };

    return attach(null);
  }, [folders, notes]);

  return {
    tree,
    isLoading: foldersQuery.isLoading || notesQuery.isLoading,
    folders,
    notes,
  };
}

export function useUploadNotebookImage() {
  const { getToken, isLoaded, isSignedIn } = useAuth();

  // NOTE: deliberately does NOT invalidate the notebook notes query. The editor
  // swaps the uploaded node's src in place, and invalidating here would refetch
  // a stale (pre-upload) copy of the note's document into the cache, which on a
  // remount re-hydrates the editor and lets the autosave overwrite the just-
  // added media. A genuine reload fetches the fresh note anyway.
  return useMutation({
    mutationFn: async ({
      noteId,
      file,
      onProgress,
    }: {
      noteId: string;
      file: File;
      onProgress?: (progress: notebookService.UploadProgress) => void;
    }) => {
      if (!isLoaded || !isSignedIn) {
        throw new Error("You must be signed in to upload notebook images");
      }

      return notebookService.uploadNotebookImage(
        () => getToken(),
        noteId,
        file,
        onProgress,
      );
    },
  });
}

export function useDeleteNotebookImage() {
  const { getToken, isLoaded, isSignedIn } = useAuth();

  // Same rationale as useUploadNotebookImage: the node is removed from the
  // editor directly, so invalidating the notes query (which could clobber the
  // editor on a remount) is unnecessary and unsafe mid-edit.
  return useMutation({
    mutationFn: async (imageId: string) => {
      if (!isLoaded || !isSignedIn) {
        throw new Error("You must be signed in to delete notebook images");
      }

      return notebookService.deleteNotebookImage(() => getToken(), imageId);
    },
  });
}
