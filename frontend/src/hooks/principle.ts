"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as principleService from "@/lib/service/principle";
import type {
  CreatePrincipleInput,
  PrincipleWithStats,
  UpdatePrincipleInput,
} from "@/lib/types/principle";

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
    },
  });
}

export function useUpdatePrinciple(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, input }: { id: string; input: UpdatePrincipleInput }) =>
      principleService.updatePrinciple(fetcher, id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: principleKey(accountId) });
    },
  });
}

export function useDeletePrinciple(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => principleService.deletePrinciple(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: principleKey(accountId) });
    },
  });
}

export function useReorderPrinciples(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (orderedIds: string[]) =>
      principleService.reorderPrinciples(fetcher, orderedIds),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: principleKey(accountId) });
    },
  });
}
