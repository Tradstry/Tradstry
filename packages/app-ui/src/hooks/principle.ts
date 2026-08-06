"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { capture, EVENTS } from "@tradstry/app-ui/lib/analytics/events";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as principleService from "@tradstry/app-ui/lib/service/principle";
import type {
  CreatePrincipleInput,
  PrincipleWithStats,
  UpdatePrincipleInput,
} from "@tradstry/app-ui/lib/types/principle";
import {
  optimisticList,
  optimisticRemove,
  optimisticUpdate,
} from "./optimistic";

const principleKey = (workspaceId: string) => ["principles", workspaceId] as const;

export function usePrinciples(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<PrincipleWithStats[]>({
    queryKey: principleKey(workspaceId ?? ""),
    queryFn: () => principleService.fetchPrinciples(fetcher, workspaceId!),
    enabled: isLoaded && isSignedIn && !!workspaceId,
  });
}

export function useCreatePrinciple(workspaceId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreatePrincipleInput) =>
      principleService.createPrinciple(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: principleKey(workspaceId) });
      capture(EVENTS.principleCreated, {});
    },
  });
}

export function useUpdatePrinciple(workspaceId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdatePrincipleInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      principleService.updatePrinciple(fetcher, id, input),
    ...optimisticUpdate<UpdateVars, PrincipleWithStats>(
      queryClient,
      principleKey(workspaceId),
      (vars) => vars.id,
      (entity, { input }) => ({ ...entity, ...input }) as PrincipleWithStats,
    ),
  });
}

export function useDeletePrinciple(workspaceId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => principleService.deletePrinciple(fetcher, id),
    ...optimisticRemove<string>(
      queryClient,
      principleKey(workspaceId),
      (id) => id,
    ),
  });
}

export function useReorderPrinciples(workspaceId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (orderedIds: string[]) =>
      principleService.reorderPrinciples(fetcher, orderedIds),
    ...optimisticList<string[], PrincipleWithStats>(
      queryClient,
      principleKey(workspaceId),
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
