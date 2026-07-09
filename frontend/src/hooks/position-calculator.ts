"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as positionCalculatorService from "@/lib/service/position-calculator";
import type {
  CreatePositionCalculatorHistoryInput,
  CreatePositionCalculatorPlanInput,
  UpdatePositionCalculatorPlanInput,
  UpsertPositionCalculatorRuleInput,
} from "@/lib/types/position-calculator";

const ruleKey = (accountId: string) =>
  ["position-calculator-rule", accountId] as const;
const HISTORY_KEY = ["position-calculator-history"] as const;
const PLANS_KEY = ["position-calculator-plans"] as const;

export function usePositionCalculatorRule(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: ruleKey(accountId ?? ""),
    queryFn: () => positionCalculatorService.fetchRule(fetcher, accountId!),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function usePositionCalculatorHistory() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: HISTORY_KEY,
    queryFn: () => positionCalculatorService.fetchHistory(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function usePositionCalculatorPlans() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: PLANS_KEY,
    queryFn: () => positionCalculatorService.fetchPlans(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useUpsertPositionCalculatorRule(accountId: string) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: UpsertPositionCalculatorRuleInput) =>
      positionCalculatorService.upsertRule(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ruleKey(accountId) });
    },
  });
}

export function useCreatePositionCalculatorHistory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreatePositionCalculatorHistoryInput) =>
      positionCalculatorService.createHistoryEntry(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: HISTORY_KEY });
    },
  });
}

export function useDeletePositionCalculatorHistory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) =>
      positionCalculatorService.deleteHistoryEntry(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: HISTORY_KEY });
    },
  });
}

export function useCreatePositionCalculatorPlan() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreatePositionCalculatorPlanInput) =>
      positionCalculatorService.createPlan(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PLANS_KEY });
    },
  });
}

export function useUpdatePositionCalculatorPlan() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      id,
      input,
    }: {
      id: string;
      input: UpdatePositionCalculatorPlanInput;
    }) => positionCalculatorService.updatePlan(fetcher, id, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PLANS_KEY });
    },
  });
}

export function useDeletePositionCalculatorPlan() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) =>
      positionCalculatorService.deletePlan(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PLANS_KEY });
    },
  });
}
