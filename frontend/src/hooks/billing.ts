"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as billingService from "@/lib/service/billing";
import type { BillingInfo, PlanId } from "@/lib/types/billing";

export const BILLING_KEY = ["billing"] as const;

/** Plan and usage. Refetched on focus because a checkout completes in a Paddle
 * overlay — the webhook lands while the user is still on the page. */
export function useBilling() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BillingInfo>({
    queryKey: BILLING_KEY,
    queryFn: () => billingService.fetchBilling(fetcher),
    enabled: isLoaded && isSignedIn,
    refetchOnWindowFocus: true,
    staleTime: 30_000,
  });
}

export function useCheckoutInfo(plan: PlanId, enabled = true) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: [...BILLING_KEY, "checkout", plan],
    queryFn: () => billingService.fetchCheckoutInfo(fetcher, plan),
    enabled: enabled && isLoaded && isSignedIn,
  });
}

export function useBillingPortal() {
  const fetcher = useGraphQL();

  return useMutation({
    mutationFn: () => billingService.createBillingPortalSession(fetcher),
    onSuccess: (url) => {
      if (url) window.location.href = url;
    },
  });
}

/** Call after a checkout completes so the new plan appears without a reload.
 * The webhook may still be in flight, hence the delayed second attempt. */
export function useRefreshBilling() {
  const queryClient = useQueryClient();

  return () => {
    queryClient.invalidateQueries({ queryKey: BILLING_KEY });
    setTimeout(() => {
      queryClient.invalidateQueries({ queryKey: BILLING_KEY });
    }, 3000);
  };
}
