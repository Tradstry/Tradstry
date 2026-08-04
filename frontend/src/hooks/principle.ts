"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { capture, EVENTS } from "@/lib/analytics/events";
import { useGraphQL } from "@/lib/client";
import * as principleService from "@/lib/service/principle";
import type {
  CreatePrincipleInput,
  PrincipleWithStats,
  UpdatePrincipleInput,
} from "@/lib/types/principle";
import {
  optimisticList,
  optimisticRemove,
  optimisticUpdate,
} from "./optimistic";

const principleKey = (accountId: string) => ["principles", accountId] as const;

export function usePrinciples(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<PrincipleWithStats[]>({
    queryKey: principleKey(accountId ?? ""),
    queryFn: () => principleService.fetchPrinciples(fetcher, accountId!),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useCreatePrinciple(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreatePrincipleInput) =>
      principleService.createPrinciple(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: principleKey(accountId) });
      capture(EVENTS.principleCreated, {});
    },
  });
}

export function useUpdatePrinciple(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdatePrincipleInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      principleService.updatePrinciple(fetcher, id, input),
    ...optimisticUpdate<UpdateVars, PrincipleWithStats>(
      queryClient,
      principleKey(accountId),
      (vars) => vars.id,
      (entity, { input }) => ({ ...entity, ...input }) as PrincipleWithStats,
    ),
  });
}

export function useDeletePrinciple(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => principleService.deletePrinciple(fetcher, id),
    ...optimisticRemove<string>(
      queryClient,
      principleKey(accountId),
      (id) => id,
    ),
  });
}

export function useReorderPrinciples(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (orderedIds: string[]) =>
      principleService.reorderPrinciples(fetcher, orderedIds),
    ...optimisticList<string[], PrincipleWithStats>(
      queryClient,
      principleKey(accountId),
      (list, orderedIds) => {
        const rank = new Map(orderedIds.map((id, i) => [id, i]));
        return [...list].sort(
          (a, b) =>
            (rank.get(a.id) ?? Number.MAX_SAFE_INTEGER) -
            (rank.get(b.id) ?? Number.MAX_SAFE_INTEGER),
        );
      },
    ),
  });
}
