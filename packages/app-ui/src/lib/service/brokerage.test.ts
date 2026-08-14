import { describe, expect, test } from "bun:test";
import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import {
	createBrokerageAccountWorkspaces,
	fetchBrokerageConnectionAccounts,
	regroupBrokerageEpisode,
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
});
