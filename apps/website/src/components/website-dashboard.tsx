"use client";

import { useAuth, useClerk, useUser } from "@clerk/nextjs";
import {
  createHttpGraphQLFetcher,
  createWebSocketGraphQLSubscriber,
  DashboardApp,
  type TradstryPlatform,
  TradstryProvider,
} from "@tradstry/app-ui";
import { usePathname, useRouter } from "next/navigation";
import { useTheme } from "next-themes";
import * as React from "react";
import { AccountDialog } from "@/components/account";
import { Countly, countlyEnabled } from "@/lib/analytics/countly";
import { SITE_URL } from "@/lib/site";

const GRAPHQL_ENDPOINT =
  process.env.NEXT_PUBLIC_BACKEND_URL ?? "http://localhost:7899/graphql";
const APP_URL = process.env.NEXT_PUBLIC_APP_URL ?? SITE_URL;
const DASHBOARD_COMPACT_METRICS =
  process.env.NEXT_PUBLIC_DASHBOARD_COMPACT_METRICS === "true";

export function WebsiteDashboard() {
  const pathname = usePathname();
  const router = useRouter();
  const auth = useAuth();
  const { user } = useUser();
  const { signOut } = useClerk();
  const { theme = "system", setTheme } = useTheme();

  const fetcher = React.useMemo(
    () =>
      createHttpGraphQLFetcher({
        endpoint: GRAPHQL_ENDPOINT,
        getToken: auth.getToken,
      }),
    [auth.getToken],
  );
  const subscriber = React.useMemo(
    () =>
      createWebSocketGraphQLSubscriber({
        endpoint: GRAPHQL_ENDPOINT,
        getToken: auth.getToken,
      }),
    [auth.getToken],
  );

  const platform = React.useMemo<TradstryPlatform>(
    () => ({
      auth: {
        isLoaded: auth.isLoaded,
        isSignedIn: auth.isSignedIn ?? false,
        getToken: auth.getToken,
      },
      user: {
        fullName: user?.fullName ?? "User",
        email: user?.primaryEmailAddress?.emailAddress ?? "",
        imageUrl: user?.imageUrl,
      },
      pathname,
      appBaseUrl:
        typeof window === "undefined" ? APP_URL : window.location.origin,
      backendBaseUrl: GRAPHQL_ENDPOINT.replace(/\/graphql\/?$/, ""),
      navigate: router.push,
      openExternal: (url) => {
        window.location.href = url;
      },
      signOut: () => signOut({ redirectUrl: "/" }),
      theme: theme === "light" || theme === "dark" ? theme : "system",
      setTheme,
      features: {
        dashboardCompactMetrics: DASHBOARD_COMPACT_METRICS,
      },
      capture: (event, properties) => {
        if (countlyEnabled()) {
          Countly.add_event({ key: event, count: 1, segmentation: properties });
        }
      },
      renderAccountDialog: (open, onOpenChange) => (
        <AccountDialog open={open} onOpenChange={onOpenChange} />
      ),
    }),
    [
      auth.getToken,
      auth.isLoaded,
      auth.isSignedIn,
      pathname,
      router.push,
      setTheme,
      signOut,
      theme,
      user,
    ],
  );

  return (
    <TradstryProvider
      platform={platform}
      fetcher={fetcher}
      subscriber={subscriber}
    >
      <DashboardApp pathname={pathname} />
    </TradstryProvider>
  );
}
