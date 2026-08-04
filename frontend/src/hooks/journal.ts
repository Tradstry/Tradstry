"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { capture, EVENTS } from "@/lib/analytics/events";
import { useGraphQL } from "@/lib/client";
import * as journalService from "@/lib/service/journal";
import type {
  CreateJournalEntryInput,
  JournalEntry,
  UpdateJournalEntryInput,
} from "@/lib/types/journal";
import type { OptimisticContext } from "./optimistic";
import { optimisticRemove, optimisticUpdate } from "./optimistic";

const JOURNAL_KEY = ["journal"] as const;

/** Trade tags/violations carry no per-link role here, so `tag_applied` reports "unknown". */
function captureLinkedTagsAndViolations(input: {
  tagIds?: string[];
  violatedPrincipleIds?: string[];
}) {
  for (const tagId of input.tagIds ?? []) {
    capture(EVENTS.tagApplied, { tagId, role: "unknown" });
  }
  for (const principleId of input.violatedPrincipleIds ?? []) {
    capture(EVENTS.violationFlagged, { principleId });
  }
}

/** Recover the deleted entry's accountId from the pre-mutation cache snapshot; `id` alone doesn't carry it. */
function findAccountId(ctx: OptimisticContext | undefined, id: string): string {
  for (const [, data] of ctx?.snapshots ?? []) {
    if (Array.isArray(data)) {
      const match = (data as JournalEntry[]).find((entry) => entry?.id === id);
      if (match?.accountId) {
        return match.accountId;
      }
    }
  }
  return "";
}

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
    onSuccess: (_data, input) => {
      queryClient.invalidateQueries({ queryKey: JOURNAL_KEY });
      capture(EVENTS.tradeLogged, {
        accountId: input.accountId,
        symbol: input.symbol,
        source: "manual",
      });
      captureLinkedTagsAndViolations(input);
    },
  });
}

export function useUpdateJournalEntry() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdateJournalEntryInput };
  // Merge the changed scalar fields for instant feedback; tags, violations and playbook
  // are relational and get their exact state from the background settle refetch.
  const optimistic = optimisticUpdate<UpdateVars, JournalEntry>(
    queryClient,
    JOURNAL_KEY,
    (vars) => vars.id,
    (entity, { input }) => ({ ...entity, ...input }) as JournalEntry,
  );

  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      journalService.updateJournalEntry(fetcher, id, input),
    // optimisticUpdate returns onMutate/onError/onSettled only, so adding onSuccess
    // here is additive, not an override.
    ...optimistic,
    onSuccess: (_data, vars) => {
      capture(EVENTS.tradeEdited, { accountId: vars.input.accountId ?? "" });
      captureLinkedTagsAndViolations(vars.input);
    },
  });
}

export function useDeleteJournalEntry() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  const optimistic = optimisticRemove<string>(
    queryClient,
    JOURNAL_KEY,
    (id) => id,
  );

  return useMutation({
    mutationFn: (id: string) => journalService.deleteJournalEntry(fetcher, id),
    ...optimistic,
    onSuccess: (_data, id, ctx) => {
      capture(EVENTS.tradeDeleted, { accountId: findAccountId(ctx, id) });
    },
  });
}
