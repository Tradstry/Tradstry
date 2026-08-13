import { describe, expect, test } from "bun:test";
import {
	calculateRiskBudget,
	calculateTrancheRisk,
	summarizePlanRisk,
} from "./position-calculator-risk";

describe("position calculator plan risk", () => {
	test("allocates a long plan by risk instead of by share count", () => {
		const riskBudget = calculateRiskBudget(10_000, 1);
		expect(riskBudget).toBe(100);
		if (riskBudget == null) throw new Error("Expected a risk budget");

		const first = calculateTrancheRisk({
			positionType: "long",
			entryPrice: 10,
			stopLoss: 9,
			riskBudget,
			riskPercent: 60,
		});
		const second = calculateTrancheRisk({
			positionType: "long",
			entryPrice: 11,
			stopLoss: 9,
			riskBudget,
			riskPercent: 40,
		});

		expect(first?.shares).toBe(60);
		expect(first?.actualRisk).toBe(60);
		expect(second?.shares).toBe(20);
		expect(second?.actualRisk).toBe(40);
		if (!first || !second) throw new Error("Expected valid tranches");

		const summary = summarizePlanRisk([
			{ ...first, targetPrice: 10 },
			{ ...second, targetPrice: 11 },
		]);
		expect(summary?.totalShares).toBe(80);
		expect(summary?.totalRisk).toBe(100);
		expect(summary?.weightedEntry).toBe(10.25);
	});

	test("recalculates short shares from each entry-to-stop distance", () => {
		const tranche = calculateTrancheRisk({
			positionType: "short",
			entryPrice: 10,
			stopLoss: 12,
			riskBudget: 200,
			riskPercent: 50,
		});

		expect(tranche?.riskPerShare).toBe(2);
		expect(tranche?.shares).toBe(50);
		expect(tranche?.actualRisk).toBe(100);
	});

	test("rounds shares down so actual risk cannot exceed the allocation", () => {
		const tranche = calculateTrancheRisk({
			positionType: "long",
			entryPrice: 10.33,
			stopLoss: 9,
			riskBudget: 100,
			riskPercent: 100,
		});

		expect(tranche?.shares).toBe(75.18);
		expect(tranche?.actualRisk).toBeLessThanOrEqual(100);
	});

	test("rejects an entry on the wrong side of the stop", () => {
		expect(
			calculateTrancheRisk({
				positionType: "long",
				entryPrice: 8,
				stopLoss: 9,
				riskBudget: 100,
				riskPercent: 100,
			}),
		).toBeNull();
	});

	test("rejects a tranche when its allocation cannot buy a fractional share", () => {
		expect(
			calculateTrancheRisk({
				positionType: "long",
				entryPrice: 200,
				stopLoss: 100,
				riskBudget: 1,
				riskPercent: 1,
			}),
		).toBeNull();
	});
});
