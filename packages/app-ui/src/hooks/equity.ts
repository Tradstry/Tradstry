"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as equityService from "@tradstry/app-ui/lib/service/equity";
import type { AnalyticsRange } from "@tradstry/app-ui/lib/types/analytics";
import type { AccountEquityHistory } from "@tradstry/app-ui/lib/types/equity";

const EQUITY_KEY = ["equity-history"] as const;

/** The chart's x-axis start. Equity is a daily series, so the range maps to a
 * plain calendar cutoff rather than the trade-time filter analytics uses. */
export function rangeToFromDate(range: AnalyticsRange): string | null {
  const now = new Date();
  const d = new Date(now);

  switch (range) {
    case "TODAY":
      break;
    case "LAST_7_DAYS":
      d.setDate(d.getDate() - 7);
      break;
    case "LAST_1_MONTH":
      d.setMonth(d.getMonth() - 1);
      break;
    case "LAST_3_MONTHS":
      d.setMonth(d.getMonth() - 3);
      break;
    case "LAST_6_MONTHS":
      d.setMonth(d.getMonth() - 6);
      break;
    case "YEAR_TO_DATE":
      d.setMonth(0, 1);
      break;
    case "LAST_1_YEAR":
      d.setFullYear(d.getFullYear() - 1);
      break;
    default:
      return null;
  }

  return d.toISOString().slice(0, 10);
}

export function useAccountEquityHistory(
  workspaceId: string | null,
  range: AnalyticsRange,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();
  const from = rangeToFromDate(range);

  return useQuery<AccountEquityHistory>({
    queryKey: [...EQUITY_KEY, workspaceId, from],
    queryFn: () => {
      if (!workspaceId) {
        throw new Error("workspace id is required");
      }
      return equityService.fetchAccountEquityHistory(
        fetcher,
        workspaceId,
        from,
      );
    },
    enabled: isLoaded && isSignedIn && Boolean(workspaceId),
    placeholderData: keepPreviousData,
  });
}

export function useRebuildAccountEquityHistory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (workspaceId: string) =>
      equityService.rebuildAccountEquityHistory(fetcher, workspaceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: EQUITY_KEY });
    },
  });
}
