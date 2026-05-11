"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as notebookService from "@/lib/service/notebook";
import type {
  CreateNotebookNoteInput,
  NotebookNote,
  UpdateNotebookNoteInput,
} from "@/lib/types/notebook";

const NOTEBOOK_KEY = ["notebook"] as const;

export function useNotebookNotes(accountId?: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<NotebookNote[]>({
    queryKey: [...NOTEBOOK_KEY, "notes", accountId ?? null],
    queryFn: () => notebookService.fetchNotebookNotes(fetcher, accountId),
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

export function useUploadNotebookImage() {
  const { getToken, isLoaded, isSignedIn } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async ({ noteId, file }: { noteId: string; file: File }) => {
      if (!isLoaded || !isSignedIn) {
        throw new Error("You must be signed in to upload notebook images");
      }

      return notebookService.uploadNotebookImage(
        () => getToken(),
        noteId,
        file,
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: NOTEBOOK_KEY });
    },
  });
}

export function useDeleteNotebookImage() {
  const { getToken, isLoaded, isSignedIn } = useAuth();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: async (imageId: string) => {
      if (!isLoaded || !isSignedIn) {
        throw new Error("You must be signed in to delete notebook images");
      }

      return notebookService.deleteNotebookImage(() => getToken(), imageId);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: NOTEBOOK_KEY });
    },
  });
}
