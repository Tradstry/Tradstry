"use client";

import { useState, useRef, useCallback, useEffect } from "react";
import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import { toast } from "sonner";
import type {
  BrokerageBalance,
  BrokerageHolding,
  BrokerageTransactionsPage,
  ConnectionPortal,
  LinkSnaptradeInput,
  SyncResult,
  TransactionFilters,
} from "@/lib/types/brokerage";
import * as brokerageService from "@/lib/service/brokerage";

const TRANSACTIONS_KEY = ["brokerage-transactions"] as const;
const HOLDINGS_KEY = ["brokerage-holdings"] as const;
const BALANCES_KEY = ["brokerage-balances"] as const;
const LINKED_TX_IDS_KEY = ["linked-brokerage-tx-ids"] as const;

export function useBrokerageTransactions(
  accountId: string | null,
  filters?: TransactionFilters,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageTransactionsPage>({
    queryKey: [...TRANSACTIONS_KEY, accountId, filters],
    queryFn: () => brokerageService.fetchTransactions(fetcher, accountId!, filters),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useBrokerageHoldings(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageHolding[]>({
    queryKey: [...HOLDINGS_KEY, accountId],
    queryFn: () => brokerageService.fetchHoldings(fetcher, accountId!),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useBrokerageBalances(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<BrokerageBalance[]>({
    queryKey: [...BALANCES_KEY, accountId],
    queryFn: () => brokerageService.fetchBalances(fetcher, accountId!),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useLinkedBrokerageTransactionIds(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<string[]>({
    queryKey: [...LINKED_TX_IDS_KEY, accountId],
    queryFn: () => brokerageService.fetchLinkedBrokerageTransactionIds(fetcher, accountId!),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useInitiateConnection() {
  const fetcher = useGraphQL();

  return useMutation<ConnectionPortal, Error, { accountId: string; brokerageId?: string; customRedirect?: string }>({
    mutationFn: ({ accountId, brokerageId, customRedirect }) =>
      brokerageService.initiateConnection(fetcher, accountId, brokerageId, customRedirect),
  });
}

export function useCompleteConnection() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<boolean, Error, { accountId: string; connectionId: string }>({
    mutationFn: ({ accountId, connectionId }) =>
      brokerageService.completeConnection(fetcher, accountId, connectionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useLinkSnaptradeAccount() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: LinkSnaptradeInput) =>
      brokerageService.linkSnaptradeAccount(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useDisconnectBrokerage() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<boolean, Error, string>({
    mutationFn: (accountId: string) =>
      brokerageService.disconnectBrokerage(fetcher, accountId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["accounts"] });
    },
  });
}

export function useSyncBrokerageData() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<SyncResult, Error, string>({
    mutationFn: (accountId: string) =>
      brokerageService.syncBrokerageData(fetcher, accountId),
    onSuccess: (_data, accountId) => {
      queryClient.invalidateQueries({ queryKey: [...TRANSACTIONS_KEY, accountId] });
      queryClient.invalidateQueries({ queryKey: [...HOLDINGS_KEY, accountId] });
      queryClient.invalidateQueries({ queryKey: [...BALANCES_KEY, accountId] });
    },
  });
}

const SYNC_STALE_MS = 5 * 60 * 1000; // 5 minutes
const SYNC_STORAGE_KEY = "brokerage-last-sync";

type SyncState = "idle" | "syncing" | "synced" | "error";

export function useAutoSync(accountId: string | null) {
  const [syncState, setSyncState] = useState<SyncState>("idle");
  const [lastSyncTime, setLastSyncTime] = useState<string | null>(null);
  const didRun = useRef(false);
  const { mutateAsync } = useSyncBrokerageData();
  const mutateRef = useRef(mutateAsync);
  mutateRef.current = mutateAsync;

  const runSync = useCallback(async () => {
    if (!accountId) return;
    setSyncState("syncing");
    try {
      await mutateRef.current(accountId);
      const now = new Date().toISOString();
      sessionStorage.setItem(SYNC_STORAGE_KEY, now);
      setLastSyncTime(now);
      setSyncState("synced");
      toast.success("Brokerage data synced");
    } catch {
      setSyncState("error");
      toast.error("Failed to sync brokerage data");
    }
  }, [accountId]);

  useEffect(() => {
    if (!accountId || didRun.current) return;
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
  }, [accountId, runSync]);

  return { syncState, lastSyncTime, retrySync: runSync };
}
