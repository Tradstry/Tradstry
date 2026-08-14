import { describe, expect, test } from "bun:test";
import {
	resolveHistoryTranches,
	trancheRisk,
} from "./position-calculator-history";
import type {
	PositionCalculatorHistoryEntry,
	PositionCalculatorPlan,
} from "./types/position-calculator";

const history = {
	id: "history-1",
	userId: "user-1",
	workspaceId: "workspace-1",
	symbol: "AAPL",
	positionType: "long",
	entryPrice: 102,
	stopLoss: 95,
	accountBalance: 10_000,
	accountRisk: 1,
	shares: 10,
	positionValue: 1_020,
	accountPct: 10.2,
	stopLossPct: 6.86,
	planId: "plan-1",
	tranches: [],
	createdAt: "2026-08-13T18:00:00Z",
} satisfies PositionCalculatorHistoryEntry;

const plan = {
	id: "plan-1",
	userId: "user-1",
	workspaceId: "workspace-1",
	symbol: "AAPL",
	positionType: "long",
	entryPrice: 102,
	stopLoss: 95,
	accountBalance: 10_000,
	accountRisk: 1,
	totalShares: 10,
	positionValue: 1_020,
	status: "completed",
	tranches: [
		{
			id: "fill-1",
			percent: 60,
			shares: 6,
			targetPrice: 100,
			status: "filled",
			filledAt: null,
		},
		{
			id: "fill-2",
			percent: 40,
			shares: 4,
			targetPrice: 105,
			status: "filled",
			filledAt: null,
		},
	],
	notes: null,
	instrumentJson: null,
	createdAt: "2026-08-13T17:00:00Z",
	updatedAt: "2026-08-13T18:00:00Z",
} satisfies PositionCalculatorPlan;

describe("position calculator history details", () => {
	test("uses the immutable snapshot before looking at plans", () => {
		const snapshot = [{ ...plan.tranches[0], id: "snapshot-fill" }];
		expect(
			resolveHistoryTranches({ ...history, tranches: snapshot }, [plan]),
		).toEqual(snapshot);
	});

	test("recovers legacy details from an exactly matching completed plan", () => {
		expect(resolveHistoryTranches(history, [plan])).toEqual(plan.tranches);
	});

	test("does not attach a different execution to a legacy history row", () => {
		expect(
			resolveHistoryTranches({ ...history, entryPrice: 103 }, [plan]),
		).toEqual([]);
	});

	test("calculates risk for filled legs but not skipped legs", () => {
		expect(trancheRisk("long", 95, plan.tranches[0])).toBe(30);
		expect(
			trancheRisk("long", 95, { ...plan.tranches[0], status: "skipped" }),
		).toBeNull();
	});
});
