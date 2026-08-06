"use client";

import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import * as React from "react";
import type {
  GraphQLFetcher,
  GraphQLSubscriber,
} from "../lib/client";
import { configureBackendBaseUrl } from "../lib/client";
import { configureCapture } from "../lib/analytics/events";

export type TradstryAuth = {
  isLoaded: boolean;
  isSignedIn: boolean;
  getToken: () => Promise<string | null>;
};

export type TradstryUser = {
  fullName: string;
  email: string;
  imageUrl?: string;
};

export type TradstryTheme = "light" | "dark" | "system";

export type TradstryPlatform = {
  auth: TradstryAuth;
  user: TradstryUser;
  pathname: string;
  appBaseUrl: string;
  backendBaseUrl: string;
  navigate: (path: string) => void;
  openExternal: (url: string) => void | Promise<void>;
  signOut: () => void | Promise<void>;
  theme: TradstryTheme;
  setTheme: (theme: TradstryTheme) => void;
  features?: {
    dashboardCompactMetrics?: boolean;
  };
  capture?: (event: string, properties: Record<string, unknown>) => void;
  renderAccountDialog?: (
    open: boolean,
    onOpenChange: (open: boolean) => void,
  ) => React.ReactNode;
};

export function platformUrl(platform: TradstryPlatform, path: string): string {
  return new URL(path, `${platform.appBaseUrl.replace(/\/$/, "")}/`).toString();
}

const PlatformContext = React.createContext<TradstryPlatform | null>(null);
const GraphQLContext = React.createContext<GraphQLFetcher | null>(null);
const SubscriptionContext = React.createContext<GraphQLSubscriber | null>(null);

export function TradstryProvider({
  platform,
  fetcher,
  subscriber,
  children,
}: {
  platform: TradstryPlatform;
  fetcher: GraphQLFetcher;
  subscriber: GraphQLSubscriber;
  children: React.ReactNode;
}) {
  configureBackendBaseUrl(platform.backendBaseUrl);
  configureCapture(platform.capture);
  const queryClientRef = React.useRef<QueryClient | null>(null);
  if (!queryClientRef.current) {
    queryClientRef.current = new QueryClient({
      defaultOptions: { queries: { staleTime: 30_000, retry: 1 } },
    });
  }

  return (
    <QueryClientProvider client={queryClientRef.current}>
      <PlatformContext value={platform}>
        <GraphQLContext value={fetcher}>
          <SubscriptionContext value={subscriber}>
            {children}
          </SubscriptionContext>
        </GraphQLContext>
      </PlatformContext>
    </QueryClientProvider>
  );
}

export function useTradstryPlatform(): TradstryPlatform {
  const platform = React.useContext(PlatformContext);
  if (!platform) throw new Error("Tradstry UI requires <TradstryProvider>");
  return platform;
}

export function useAuth(): TradstryAuth {
  return useTradstryPlatform().auth;
}

export function useGraphQL(): GraphQLFetcher {
  const fetcher = React.useContext(GraphQLContext);
  if (!fetcher) throw new Error("Tradstry UI requires <TradstryProvider>");
  return fetcher;
}

export function useGraphQLSubscription(): GraphQLSubscriber {
  const subscriber = React.useContext(SubscriptionContext);
  if (!subscriber) throw new Error("Tradstry UI requires <TradstryProvider>");
  return subscriber;
}
