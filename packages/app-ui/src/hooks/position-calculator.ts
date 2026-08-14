"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as positionCalculatorService from "@tradstry/app-ui/lib/service/position-calculator";
import type {
  CreatePositionCalculatorHistoryInput,
  CreatePositionCalculatorPlanInput,
  PositionCalculatorPlan,
  UpdatePositionCalculatorPlanInput,
  UpsertPositionCalculatorRuleInput,
} from "@tradstry/app-ui/lib/types/position-calculator";
import { useAuth } from "@tradstry/app-ui/platform";
import { optimisticRemove, optimisticUpdate } from "./optimistic";

const ruleKey = (workspaceId: string) =>
  ["position-calculator-rule", workspaceId] as const;
const historyKey = (workspaceId: string) =>
  ["position-calculator-history", workspaceId] as const;
const plansKey = (workspaceId: string) =>
  ["position-calculator-plans", workspaceId] as const;
const tradeReviewKey = (workspaceId: string) =>
  ["trade-review-inbox", workspaceId] as const;

export function useTradeReviewInbox(enabled = true) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();
  const workspace = useActiveWorkspace();
  return useQuery({
    queryKey: tradeReviewKey(workspace?.id ?? ""),
    queryFn: () => positionCalculatorService.fetchTradeReviewInbox(fetcher, workspace!.id),
    enabled: enabled && isLoaded && isSignedIn && !!workspace,
    refetchInterval: 15_000,
  });
}

export function useRequestPlanExecutionCheck() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();
  return useMutation({
    mutationFn: (planId: string) => positionCalculatorService.requestExecutionCheck(fetcher, planId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: tradeReviewKey(workspace?.id ?? "") }),
  });
}

export function useConfirmTradeMatch() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();
  return useMutation({
    mutationFn: ({ episodeId, planId }: { episodeId: string; planId: string }) =>
      positionCalculatorService.confirmTradeMatch(fetcher, episodeId, planId),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: tradeReviewKey(workspace?.id ?? "") }),
  });
}

export function useFinalizeTradeReview() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();
  return useMutation({
    mutationFn: positionCalculatorService.finalizeTradeReview.bind(null, fetcher),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: tradeReviewKey(workspace?.id ?? "") }),
  });
}

export function usePublishTradeReview() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();
  return useMutation({
    mutationFn: (matchId: string) => positionCalculatorService.publishTradeReview(fetcher, matchId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: tradeReviewKey(workspace?.id ?? "") });
      queryClient.invalidateQueries({ queryKey: historyKey(workspace?.id ?? "") });
      queryClient.invalidateQueries({ queryKey: plansKey(workspace?.id ?? "") });
    },
  });
}

export function usePositionCalculatorRule(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: ruleKey(workspaceId ?? ""),
    queryFn: () => positionCalculatorService.fetchRule(fetcher, workspaceId!),
    enabled: isLoaded && isSignedIn && !!workspaceId,
  });
}

export function usePositionCalculatorHistory() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();
  const workspace = useActiveWorkspace();

  return useQuery({
    queryKey: historyKey(workspace?.id ?? ""),
    queryFn: () =>
      positionCalculatorService.fetchHistory(fetcher, workspace!.id),
    enabled: isLoaded && isSignedIn && !!workspace,
  });
}

export function usePositionCalculatorPlans() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();
  const workspace = useActiveWorkspace();

  return useQuery({
    queryKey: plansKey(workspace?.id ?? ""),
    queryFn: () => positionCalculatorService.fetchPlans(fetcher, workspace!.id),
    enabled: isLoaded && isSignedIn && !!workspace,
  });
}

export function useUpsertPositionCalculatorRule(workspaceId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UpsertPositionCalculatorRuleInput) =>
      positionCalculatorService.upsertRule(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ruleKey(workspaceId) });
    },
  });
}

export function useCreatePositionCalculatorHistory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();

  return useMutation({
    mutationFn: (
      input: Omit<CreatePositionCalculatorHistoryInput, "workspaceId">,
    ) =>
      positionCalculatorService.createHistoryEntry(fetcher, {
        ...input,
        workspaceId: workspace!.id,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: historyKey(workspace?.id ?? ""),
      });
    },
  });
}

export function useDeletePositionCalculatorHistory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();

  return useMutation({
    mutationFn: (id: string) =>
      positionCalculatorService.deleteHistoryEntry(fetcher, id),
    ...optimisticRemove<string>(
      queryClient,
      historyKey(workspace?.id ?? ""),
      (id) => id,
    ),
  });
}

export function useCreatePositionCalculatorPlan() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();

  return useMutation({
    mutationFn: (
      input: Omit<CreatePositionCalculatorPlanInput, "workspaceId">,
    ) =>
      positionCalculatorService.createPlan(fetcher, {
        ...input,
        workspaceId: workspace!.id,
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({
        queryKey: plansKey(workspace?.id ?? ""),
      });
    },
  });
}

export function useUpdatePositionCalculatorPlan() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();

  type UpdateVars = { id: string; input: UpdatePositionCalculatorPlanInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      positionCalculatorService.updatePlan(fetcher, id, input),
    ...optimisticUpdate<UpdateVars, PositionCalculatorPlan>(
      queryClient,
      plansKey(workspace?.id ?? ""),
      (vars) => vars.id,
      (entity, { input }) => {
        const tranches = input.tranches
          ? entity.tranches.map((tranche) => {
              const update = input.tranches?.find(
                (candidate) => candidate.id === tranche.id,
              );
              return update ? { ...tranche, ...update } : tranche;
            })
          : entity.tranches;
        const notes = input.clearNotes
          ? null
          : input.notes !== undefined
            ? input.notes
            : entity.notes;

        return {
          ...entity,
          ...(input.status !== undefined ? { status: input.status } : {}),
          tranches,
          notes,
        };
      },
    ),
  });
}

export function useDeletePositionCalculatorPlan() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const workspace = useActiveWorkspace();

  return useMutation({
    mutationFn: (id: string) =>
      positionCalculatorService.deletePlan(fetcher, id),
    ...optimisticRemove<string>(
      queryClient,
      plansKey(workspace?.id ?? ""),
      (id) => id,
    ),
  });
}
