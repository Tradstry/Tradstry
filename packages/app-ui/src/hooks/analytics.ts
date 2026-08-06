"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as analyticsService from "@tradstry/app-ui/lib/service/analytics";
import type {
  AdvancedAnalytics,
  AnalyticsTimeFilterInput,
  CalendarAnalytics,
  JournalAnalytics,
} from "@tradstry/app-ui/lib/types/analytics";

const ANALYTICS_KEY = ["analytics"] as const;

export function useAdvancedAnalytics(
  workspaceId: string | null,
  timeFilter: AnalyticsTimeFilterInput,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<AdvancedAnalytics>({
    queryKey: [
      ...ANALYTICS_KEY,
      "advanced",
      workspaceId,
      timeFilter.range,
      timeFilter.startDate ?? null,
      timeFilter.endDate ?? null,
    ],
    queryFn: () => {
      if (!workspaceId) {
        throw new Error("workspace id is required");
      }

      return analyticsService.fetchAdvancedAnalytics(
        fetcher,
        workspaceId,
        timeFilter,
      );
    },
    enabled: isLoaded && isSignedIn && !!workspaceId,
    placeholderData: keepPreviousData,
  });
}

export function useJournalAnalytics(
  workspaceId: string | null,
  timeFilter: AnalyticsTimeFilterInput,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<JournalAnalytics>({
    queryKey: [
      ...ANALYTICS_KEY,
      "journal",
      workspaceId,
      timeFilter.range,
      timeFilter.startDate ?? null,
      timeFilter.endDate ?? null,
    ],
    queryFn: () => {
      if (!workspaceId) {
        throw new Error("workspace id is required");
      }

      return analyticsService.fetchJournalAnalytics(
        fetcher,
        workspaceId,
        timeFilter,
      );
    },
    enabled: isLoaded && isSignedIn && !!workspaceId,
    placeholderData: keepPreviousData,
  });
}

export function useCalendarAnalytics(
  workspaceId: string | null,
  year: number,
  month: number,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<CalendarAnalytics>({
    queryKey: [...ANALYTICS_KEY, "calendar", workspaceId, year, month],
    queryFn: () => {
      if (!workspaceId) {
        throw new Error("workspace id is required");
      }

      return analyticsService.fetchCalendarAnalytics(
        fetcher,
        workspaceId,
        year,
        month,
      );
    },
    enabled: isLoaded && isSignedIn && !!workspaceId,
    placeholderData: keepPreviousData,
  });
}
