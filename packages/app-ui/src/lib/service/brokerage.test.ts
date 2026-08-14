import { describe, expect, test } from "bun:test";
import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import {
	createBrokerageAccountWorkspaces,
	fetchBrokerageConnectionAccounts,
	fetchBrokerageReconciliation,
	fetchBrokerageSyncOutcome,
	regroupBrokerageEpisode,
	reportBrokerageDataIssue,
} from "./brokerage";

describe("brokerage account workspace service", () => {
	test("requests the accounts exposed by a workspace connection", async () => {
		const calls: Array<{
			query: string;
			variables?: Record<string, unknown>;
		}> = [];
		const fetcher = (async <T>(
			query: string,
			variables?: Record<string, unknown>,
		) => {
			calls.push({ query, variables });
			return {
				brokerageConnectionAccounts: [
					{
						id: "margin",
						name: "Individual Margin",
						institutionName: "Webull",
						linkedWorkspaceId: null,
						linkedWorkspaceName: null,
						current: false,
					},
				],
			} as T;
		}) as GraphQLFetcher;

		const accounts = await fetchBrokerageConnectionAccounts(
			fetcher,
			"workspace",
		);

		expect(accounts[0]?.id).toBe("margin");
		expect(calls[0]?.query).toContain("brokerageConnectionAccounts");
		expect(calls[0]?.variables).toEqual({ workspaceId: "workspace" });
	});

	test("sends only the selected account ids for workspace creation", async () => {
		const calls: Array<{
			query: string;
			variables?: Record<string, unknown>;
		}> = [];
		const fetcher = (async <T>(
			query: string,
			variables?: Record<string, unknown>,
		) => {
			calls.push({ query, variables });
			return {
				createBrokerageAccountWorkspaces: [
					{
						id: "created-workspace",
						name: "Individual Margin",
						snaptradeAccountId: "margin",
					},
				],
			} as T;
		}) as GraphQLFetcher;

		const created = await createBrokerageAccountWorkspaces(
			fetcher,
			"workspace",
			["margin", "events"],
		);

		expect(created).toHaveLength(1);
		expect(calls[0]?.query).toContain("createBrokerageAccountWorkspaces");
		expect(calls[0]?.variables).toEqual({
			workspaceId: "workspace",
			snaptradeAccountIds: ["margin", "events"],
		});
	});

	test("persists a corrected fill grouping without changing broker records", async () => {
		const calls: Array<{
			query: string;
			variables?: Record<string, unknown>;
		}> = [];
		const fetcher = (async <T>(
			query: string,
			variables?: Record<string, unknown>,
		) => {
			calls.push({ query, variables });
			return { regroupBrokerageEpisode: "episode" } as T;
		}) as GraphQLFetcher;

		const episodeId = await regroupBrokerageEpisode(fetcher, "episode", [
			"fill-1",
			"fill-2",
		]);

		expect(episodeId).toBe("episode");
		expect(calls[0]?.query).toContain("regroupBrokerageEpisode");
		expect(calls[0]?.variables).toEqual({
			episodeId: "episode",
			transactionIds: ["fill-1", "fill-2"],
		});
	});

	test("requests the persisted broker-to-workspace comparison", async () => {
		const calls: Array<{
			query: string;
			variables?: Record<string, unknown>;
		}> = [];
		const fetcher = (async <T>(
			query: string,
			variables?: Record<string, unknown>,
		) => {
			calls.push({ query, variables });
			return {
				brokerageReconciliation: {
					diagnosticId: "diag-webull-cash",
					transactionStatus: "matched",
					brokerTransactionCount: 18,
					localTransactionCount: 18,
					portfolioStatus: "matched",
				},
			} as T;
		}) as GraphQLFetcher;

		const result = await fetchBrokerageReconciliation(fetcher, "workspace");

		expect(result?.diagnosticId).toBe("diag-webull-cash");
		expect(calls[0]?.query).toContain("brokerageReconciliation");
		expect(calls[0]?.query).toContain("missingTransactionCount");
		expect(calls[0]?.query).toContain("balanceDiscrepancyCount");
		expect(calls[0]?.variables).toEqual({ workspaceId: "workspace" });
	});

	test("requests the backend scheduler's next sync instant", async () => {
		const calls: Array<{
			query: string;
			variables?: Record<string, unknown>;
		}> = [];
		const fetcher = (async <T>(
			query: string,
			variables?: Record<string, unknown>,
		) => {
			calls.push({ query, variables });
			return {
				brokerageSyncOutcome: {
					status: "completed",
					nextScheduledAt: "2026-08-14T14:00:00Z",
				},
			} as T;
		}) as GraphQLFetcher;

		const result = await fetchBrokerageSyncOutcome(fetcher, "workspace");

		expect(result?.nextScheduledAt).toBe("2026-08-14T14:00:00Z");
		expect(calls[0]?.query).toContain("nextScheduledAt");
		expect(calls[0]?.variables).toEqual({ workspaceId: "workspace" });
	});

	test("reports only the user-selected category and note", async () => {
		const calls: Array<{
			query: string;
			variables?: Record<string, unknown>;
		}> = [];
		const fetcher = (async <T>(
			query: string,
			variables?: Record<string, unknown>,
		) => {
			calls.push({ query, variables });
			return {
				reportBrokerageDataIssue: {
					id: "report-1",
					diagnosticId: "diag-1",
					createdAt: "2026-08-14T10:00:00Z",
				},
			} as T;
		}) as GraphQLFetcher;

		const result = await reportBrokerageDataIssue(fetcher, {
			workspaceId: "workspace",
			category: "balances",
			note: "Cash is different",
		});

		expect(result.id).toBe("report-1");
		expect(calls[0]?.query).toContain("reportBrokerageDataIssue");
		expect(calls[0]?.variables).toEqual({
			input: {
				workspaceId: "workspace",
				category: "balances",
				note: "Cash is different",
			},
		});
	});
});
