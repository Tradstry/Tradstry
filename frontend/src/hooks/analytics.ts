"use client";

import { useAuth } from "@clerk/nextjs";
import { useQuery } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as analyticsService from "@/lib/service/analytics";
import type {
  AdvancedAnalytics,
  AnalyticsTimeFilterInput,
  CalendarAnalytics,
  JournalAnalytics,
} from "@/lib/types/analytics";

const ANALYTICS_KEY = ["analytics"] as const;

export function useAdvancedAnalytics(
  accountId: string | null,
  timeFilter: AnalyticsTimeFilterInput,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<AdvancedAnalytics>({
    queryKey: [
      ...ANALYTICS_KEY,
      "advanced",
      accountId,
      timeFilter.range,
      timeFilter.startDate ?? null,
      timeFilter.endDate ?? null,
    ],
    queryFn: () => {
      if (!accountId) {
        throw new Error("account id is required");
      }

      return analyticsService.fetchAdvancedAnalytics(
        fetcher,
        accountId,
        timeFilter,
      );
    },
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useJournalAnalytics(
  accountId: string | null,
  timeFilter: AnalyticsTimeFilterInput,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<JournalAnalytics>({
    queryKey: [
      ...ANALYTICS_KEY,
      "journal",
      accountId,
      timeFilter.range,
      timeFilter.startDate ?? null,
      timeFilter.endDate ?? null,
    ],
    queryFn: () => {
      if (!accountId) {
        throw new Error("account id is required");
      }

      return analyticsService.fetchJournalAnalytics(
        fetcher,
        accountId,
        timeFilter,
      );
    },
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useCalendarAnalytics(
  accountId: string | null,
  year: number,
  month: number,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<CalendarAnalytics>({
    queryKey: [...ANALYTICS_KEY, "calendar", accountId, year, month],
    queryFn: () => {
      if (!accountId) {
        throw new Error("account id is required");
      }

      return analyticsService.fetchCalendarAnalytics(
        fetcher,
        accountId,
        year,
        month,
      );
    },
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}
