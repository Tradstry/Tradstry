"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import {
  keepPreviousData,
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { capture, EVENTS } from "@tradstry/app-ui/lib/analytics/events";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as brokerageService from "@tradstry/app-ui/lib/service/brokerage";
import type {
  BrokerageBalance,
  BrokerageHolding,
  BrokerageTransaction,
  BrokerageTransactionsPage,
  ConnectionPortal,
  PendingTrade,
  SyncResult,
  TransactionFilters,
} from "@tradstry/app-ui/lib/types/brokerage";
import type { Workspace } from "@tradstry/app-ui/lib/types/workspaces";

const TRANSACTIONS_KEY = ["brokerage-transactions"] as const;
const HOLDINGS_KEY = ["brokerage-holdings"] as const;
const BALANCES_KEY = ["brokerage-balances"] as const;
const LINKED_TX_IDS_KEY = ["linked-brokerage-tx-ids"] as const;
const PENDING_TRADES_KEY = ["pending-trades"] as const;
const WORKSPACES_KEY = ["workspaces"] as const;

export function useBrokerageTransactions(
  workspaceId: string | null,
  filters?: TransactionFilters,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageTransactionsPage>({
    queryKey: [...TRANSACTIONS_KEY, workspaceId, filters],
    queryFn: () =>
      brokerageService.fetchTransactions(fetcher, workspaceId!, filters),
    enabled: isLoaded && isSignedIn && !!workspaceId,
    placeholderData: keepPreviousData,
  });
}

export function useBrokerageHoldings(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageHolding[]>({
    queryKey: [...HOLDINGS_KEY, workspaceId],
    queryFn: () => brokerageService.fetchHoldings(fetcher, workspaceId!),
    enabled: isLoaded && isSignedIn && !!workspaceId,
  });
}

export function useBrokerageBalances(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageBalance[]>({
    queryKey: [...BALANCES_KEY, workspaceId],
    queryFn: () => brokerageService.fetchBalances(fetcher, workspaceId!),
    enabled: isLoaded && isSignedIn && !!workspaceId,
  });
}

export function useLinkedBrokerageTransactionIds(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<string[]>({
    queryKey: [...LINKED_TX_IDS_KEY, workspaceId],
    queryFn: () =>
      brokerageService.fetchLinkedBrokerageTransactionIds(
        fetcher,
        workspaceId!,
      ),
    enabled: isLoaded && isSignedIn && !!workspaceId,
  });
}

/**
 * Hydrate full transaction objects from a list of ids. Used by the multi-select
 * merge flow so a selection that spans several server-side pages resolves to all
 * of its transactions, not just the ones on the current page. Shares its cache
 * key with the merge modal's prefill query.
 */
export function useBrokerageTransactionsByIds(ids: string[]) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageTransaction[]>({
    queryKey: ["brokerage-tx-by-ids", ids],
    queryFn: () =>
      brokerageService.fetchBrokerageTransactionsByIds(fetcher, ids),
    enabled: isLoaded && isSignedIn && ids.length > 0,
  });
}

export function usePendingTrades(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<PendingTrade[]>({
    queryKey: [...PENDING_TRADES_KEY, workspaceId],
    queryFn: () => brokerageService.fetchPendingTrades(fetcher, workspaceId!),
    enabled: isLoaded && isSignedIn && !!workspaceId,
    staleTime: 30_000,
  });
}

export function useInitiateConnection() {
  const fetcher = useGraphQL();

  return useMutation<
    ConnectionPortal,
    Error,
    {
      workspaceId: string;
      brokerageId?: string;
      customRedirect?: string;
      reconnect?: boolean;
    }
  >({
    mutationFn: ({ workspaceId, brokerageId, customRedirect, reconnect }) =>
      brokerageService.initiateConnection(
        fetcher,
        workspaceId,
        brokerageId,
        customRedirect,
        reconnect,
      ),
  });
}

export function useCompleteConnection() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<
    boolean,
    Error,
    { workspaceId: string; connectionId: string }
  >({
    mutationFn: ({ workspaceId, connectionId }) =>
      brokerageService.completeConnection(fetcher, workspaceId, connectionId),
    onSuccess: (_data, { workspaceId }) => {
      // Read before invalidating; the broker name only exists on the cached account.
      const broker = queryClient
        .getQueryData<Workspace[]>(["workspaces"])
        ?.find((account) => account.id === workspaceId)?.broker;
      capture(EVENTS.brokerageConnected, { broker: broker ?? "unknown" });
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });
}

export function useDisconnectBrokerage() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<boolean, Error, string>({
    mutationFn: (workspaceId: string) =>
      brokerageService.disconnectBrokerage(fetcher, workspaceId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });
}

export function useSyncBrokerageData() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<SyncResult, Error, string>({
    mutationFn: (workspaceId: string) =>
      brokerageService.syncBrokerageData(fetcher, workspaceId),
    onSuccess: (_data, workspaceId) => {
      queryClient.invalidateQueries({
        queryKey: [...TRANSACTIONS_KEY, workspaceId],
      });
      queryClient.invalidateQueries({
        queryKey: [...HOLDINGS_KEY, workspaceId],
      });
      queryClient.invalidateQueries({
        queryKey: [...BALANCES_KEY, workspaceId],
      });
    },
    // A sync that fails on stale credentials flags the connection disabled
    // server-side; without this refetch the card keeps showing the stale
    // "connected" state and never offers Reconnect.
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: WORKSPACES_KEY });
    },
  });
}

const SYNC_STALE_MS = 5 * 60 * 1000; // 5 minutes
const SYNC_STORAGE_KEY = "brokerage-last-sync";

type SyncState = "idle" | "syncing" | "synced" | "error";

export function useAutoSync(workspaceId: string | null) {
  const [syncState, setSyncState] = useState<SyncState>("idle");
  const [lastSyncTime, setLastSyncTime] = useState<string | null>(null);
  const didRun = useRef(false);
  const { mutateAsync } = useSyncBrokerageData();
  const mutateRef = useRef(mutateAsync);
  mutateRef.current = mutateAsync;

  const runSync = useCallback(async () => {
    if (!workspaceId) return;
    setSyncState("syncing");
    try {
      await mutateRef.current(workspaceId);
      const now = new Date().toISOString();
      sessionStorage.setItem(SYNC_STORAGE_KEY, now);
      setLastSyncTime(now);
      setSyncState("synced");
      toast.success("Brokerage data synced");
    } catch (err) {
      setSyncState("error");
      toast.error(
        err instanceof Error ? err.message : "Failed to sync brokerage data",
      );
    }
  }, [workspaceId]);

  useEffect(() => {
    if (!workspaceId || didRun.current) return;
    didRun.current = true;

    const stored = sessionStorage.getItem(SYNC_STORAGE_KEY);
    if (stored) {
      const elapsed = Date.now() - new Date(stored).getTime();
      if (elapsed < SYNC_STALE_MS) {
        setLastSyncTime(stored);
        setSyncState("synced");
        return;
      }
    }
    runSync();
  }, [workspaceId, runSync]);

  return { syncState, lastSyncTime, retrySync: runSync };
}
