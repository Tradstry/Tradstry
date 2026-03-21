"use client";

import { useAuth } from "@clerk/nextjs";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { createContext, useContext, useMemo, useRef } from "react";
import {
  type GraphQLFetcher,
  type GraphQLSubscriber,
  createGraphQLFetcher,
  createGraphQLSubscriber,
} from "./backend-connection";

const GraphQLContext = createContext<GraphQLFetcher | null>(null);
const GraphQLSubscriptionContext = createContext<GraphQLSubscriber | null>(null);

export function useGraphQL(): GraphQLFetcher {
  const client = useContext(GraphQLContext);
  if (!client) {
    throw new Error("useGraphQL must be used within <GraphQLProvider>");
  }
  return client;
}

export function useGraphQLSubscription(): GraphQLSubscriber {
  const client = useContext(GraphQLSubscriptionContext);
  if (!client) {
    throw new Error(
      "useGraphQLSubscription must be used within <GraphQLProvider>",
    );
  }
  return client;
}

export function GraphQLProvider({ children }: { children: React.ReactNode }) {
  const { getToken } = useAuth();
  const queryClientRef = useRef<QueryClient | null>(null);

  if (!queryClientRef.current) {
    queryClientRef.current = new QueryClient({
      defaultOptions: {
        queries: { staleTime: 30_000, retry: 1 },
      },
    });
  }

  const fetcher = useMemo(
    () => createGraphQLFetcher(() => getToken()),
    [getToken],
  );
  const subscriber = useMemo(
    () => createGraphQLSubscriber(() => getToken()),
    [getToken],
  );

  return (
    <QueryClientProvider client={queryClientRef.current}>
      <GraphQLContext value={fetcher}>
        <GraphQLSubscriptionContext value={subscriber}>
          {children}
        </GraphQLSubscriptionContext>
      </GraphQLContext>
    </QueryClientProvider>
  );
}
