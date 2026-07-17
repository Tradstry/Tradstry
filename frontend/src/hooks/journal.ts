"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as journalService from "@/lib/service/journal";
import type {
  CreateJournalEntryInput,
  JournalEntry,
  UpdateJournalEntryInput,
} from "@/lib/types/journal";
import { optimisticRemove, optimisticUpdate } from "./optimistic";

const JOURNAL_KEY = ["journal"] as const;

export function useJournalEntries() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<JournalEntry[]>({
    queryKey: JOURNAL_KEY,
    queryFn: () => journalService.fetchJournalEntries(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useJournalEntriesForAccount(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<JournalEntry[]>({
    queryKey: [...JOURNAL_KEY, "account", accountId],
    queryFn: async () => {
      const entries = await journalService.fetchJournalEntries(fetcher);
      if (!accountId) {
        return [];
      }

      return entries.filter((entry) => entry.accountId === accountId);
    },
    enabled: isLoaded && isSignedIn,
  });
}

export function useJournalEntry(id: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<JournalEntry | null>({
    queryKey: [...JOURNAL_KEY, id],
    queryFn: () => {
      if (!id) {
        throw new Error("journal entry id is required");
      }
      return journalService.fetchJournalEntry(fetcher, id);
    },
    enabled: isLoaded && isSignedIn && !!id,
  });
}

export function useCreateJournalEntry() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateJournalEntryInput) =>
      journalService.createJournalEntry(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: JOURNAL_KEY });
    },
  });
}

export function useUpdateJournalEntry() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdateJournalEntryInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      journalService.updateJournalEntry(fetcher, id, input),
    // Merge the changed scalar fields for instant feedback; tags, violations and playbook
    // are relational and get their exact state from the background settle refetch.
    ...optimisticUpdate<UpdateVars, JournalEntry>(
      queryClient,
      JOURNAL_KEY,
      (vars) => vars.id,
      (entity, { input }) => ({ ...entity, ...input }) as JournalEntry,
    ),
  });
}

export function useDeleteJournalEntry() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => journalService.deleteJournalEntry(fetcher, id),
    ...optimisticRemove<string>(queryClient, JOURNAL_KEY, (id) => id),
  });
}
