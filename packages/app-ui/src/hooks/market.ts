"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
	useGraphQL,
	useGraphQLSubscription,
} from "@tradstry/app-ui/lib/client";
import * as service from "@tradstry/app-ui/lib/service/market";
import type {
	MarketPriceUpdate,
	MarketQuote,
} from "@tradstry/app-ui/lib/types/market";
import { useEffect, useMemo } from "react";

const researchKey = (kind: string, symbol: string) =>
	["market", kind, symbol] as const;
const workspaceKey = (kind: string, workspaceId: string | null) =>
	["market", kind, workspaceId] as const;

export function useMarketQuotes(symbols: string[]) {
	const fetcher = useGraphQL();
	const subscriber = useGraphQLSubscription();
	const queryClient = useQueryClient();
	const symbolKey = symbols.join(",");
	const subscriptionSymbols = useMemo(
		() => symbolKey.split(",").filter(Boolean),
		[symbolKey],
	);
	const queryKey = useMemo(
		() => ["market", "quotes", subscriptionSymbols] as const,
		[subscriptionSymbols],
	);
	const query = useQuery({
		queryKey,
		queryFn: () => service.fetchQuotes(fetcher, subscriptionSymbols),
		enabled: subscriptionSymbols.length > 0,
		refetchInterval: 30_000,
	});

	useEffect(() => {
		if (subscriptionSymbols.length === 0) return;
		return subscriber<{ marketPriceUpdates: MarketPriceUpdate }>(
			service.MARKET_PRICE_UPDATES_SUBSCRIPTION,
			{ symbols: subscriptionSymbols },
			{
				onMessage: ({ marketPriceUpdates: update }) => {
					queryClient.setQueryData<{
						quotes: MarketQuote[];
						errors: { symbol: string; message: string }[];
						fetchedAt: string;
					}>(queryKey, (current) =>
						current
							? {
									...current,
									quotes: current.quotes.map((quote) =>
										quote.symbol === update.symbol
											? { ...quote, ...update, isStale: false }
											: quote,
									),
								}
							: current,
					);
				},
			},
		);
	}, [queryClient, queryKey, subscriber, subscriptionSymbols]);

	return query;
}

export function useMarketSearch(query: string) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: ["market", "search", query],
		queryFn: () => service.search(fetcher, query),
		enabled: query.trim().length >= 2,
		staleTime: 5 * 60_000,
	});
}

export function useMarketChart(symbol: string, range: string) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: [...researchKey("chart", symbol), range],
		queryFn: () => service.fetchChart(fetcher, symbol, range),
		enabled: !!symbol,
	});
}

export function useMarketNews(symbol: string) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: researchKey("news", symbol),
		queryFn: () => service.fetchNews(fetcher, symbol),
		enabled: !!symbol,
	});
}

export function useMarketFinancials(symbol: string) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: researchKey("financials", symbol),
		queryFn: () => service.fetchFinancials(fetcher, symbol),
		enabled: !!symbol,
		staleTime: 60 * 60_000,
	});
}

export function useMarketCompany(symbol: string) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: researchKey("company", symbol),
		queryFn: () => service.fetchCompany(fetcher, symbol),
		enabled: !!symbol,
		staleTime: 60 * 60_000,
	});
}

export function useMarketTranscriptList(symbol: string) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: researchKey("transcripts", symbol),
		queryFn: () => service.fetchTranscriptList(fetcher, symbol),
		enabled: !!symbol,
		staleTime: 60 * 60_000,
	});
}

export function useMarketTranscript(
	symbol: string,
	quarter?: number,
	year?: number,
) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: ["market", "transcript", symbol, quarter, year],
		queryFn: () => {
			if (quarter === undefined || year === undefined)
				return Promise.resolve(null);
			return service.fetchTranscript(fetcher, symbol, quarter, year);
		},
		enabled: !!symbol && !!quarter && !!year,
		staleTime: Infinity,
	});
}

export function useMarketWatchlists(workspaceId: string | null) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: workspaceKey("watchlists", workspaceId),
		queryFn: () =>
			workspaceId
				? service.fetchWatchlists(fetcher, workspaceId)
				: Promise.resolve([]),
		enabled: !!workspaceId,
	});
}

export function useCreateMarketWatchlist(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: (name: string) => {
			if (!workspaceId) throw new Error("Select a workspace first");
			return service.createWatchlist(fetcher, workspaceId, name);
		},
		onSuccess: () =>
			qc.invalidateQueries({
				queryKey: workspaceKey("watchlists", workspaceId),
			}),
	});
}

export function useAddMarketWatchlistSymbol(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: ({
			watchlistId,
			symbol,
		}: {
			watchlistId: string;
			symbol: string;
		}) => service.addWatchlistSymbol(fetcher, watchlistId, symbol),
		onSuccess: () =>
			qc.invalidateQueries({
				queryKey: workspaceKey("watchlists", workspaceId),
			}),
	});
}

export function useRemoveMarketWatchlistSymbol(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: ({
			watchlistId,
			symbol,
		}: {
			watchlistId: string;
			symbol: string;
		}) => service.removeWatchlistSymbol(fetcher, watchlistId, symbol),
		onSuccess: () =>
			qc.invalidateQueries({
				queryKey: workspaceKey("watchlists", workspaceId),
			}),
	});
}

export function useMarketReports(workspaceId: string | null) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: workspaceKey("reports", workspaceId),
		queryFn: () =>
			workspaceId
				? service.fetchReports(fetcher, workspaceId)
				: Promise.resolve([]),
		enabled: !!workspaceId,
	});
}

export function useGenerateMarketReport(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: ({ symbol, focus }: { symbol: string; focus?: string }) => {
			if (!workspaceId) throw new Error("Select a workspace first");
			return service.generateReport(fetcher, workspaceId, symbol, focus);
		},
		onSuccess: () =>
			qc.invalidateQueries({ queryKey: workspaceKey("reports", workspaceId) }),
	});
}

export function useMarketMonitors(workspaceId: string | null) {
	const fetcher = useGraphQL();
	return useQuery({
		queryKey: workspaceKey("monitors", workspaceId),
		queryFn: () =>
			workspaceId
				? service.fetchMonitors(fetcher, workspaceId)
				: Promise.resolve([]),
		enabled: !!workspaceId,
	});
}

export function useCreateMarketMonitor(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: (input: {
			symbol: string;
			name: string;
			condition: "ABOVE" | "BELOW";
			threshold: number;
		}) => {
			if (!workspaceId) throw new Error("Select a workspace first");
			return service.createMonitor(fetcher, workspaceId, input);
		},
		onSuccess: () =>
			qc.invalidateQueries({ queryKey: workspaceKey("monitors", workspaceId) }),
	});
}

export function useDeleteMarketMonitor(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: service.deleteMonitor.bind(null, fetcher),
		onSuccess: () =>
			qc.invalidateQueries({ queryKey: workspaceKey("monitors", workspaceId) }),
	});
}

export function useEvaluateMarketMonitors(workspaceId: string | null) {
	const fetcher = useGraphQL();
	const qc = useQueryClient();
	return useMutation({
		mutationFn: () => {
			if (!workspaceId) throw new Error("Select a workspace first");
			return service.evaluateMonitors(fetcher, workspaceId);
		},
		onSuccess: () =>
			qc.invalidateQueries({ queryKey: workspaceKey("monitors", workspaceId) }),
	});
}
