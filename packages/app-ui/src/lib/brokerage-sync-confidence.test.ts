import { describe, expect, test } from "bun:test";
import {
	formatSyncTimestamp,
	syncConfidenceState,
} from "./brokerage-sync-confidence";
import type { BrokerageSyncOutcome } from "./types/brokerage";

function outcome(
	status: BrokerageSyncOutcome["status"],
	overrides: Partial<BrokerageSyncOutcome> = {},
): BrokerageSyncOutcome {
	return {
		diagnosticId: "attempt-1",
		status,
		error: null,
		startedAt: "2026-08-13T10:00:00Z",
		finishedAt: "2026-08-13T10:00:10Z",
		succeededAt: status === "completed" ? "2026-08-13T10:00:10Z" : null,
		transactionsSynced: 2,
		holdingsSynced: 3,
		balancesSynced: 1,
		...overrides,
	};
}

describe("brokerage sync confidence", () => {
	test("prioritizes reauthorization over the stored sync result", () => {
		const state = syncConfidenceState(outcome("completed"), true, false);
		expect(state.label).toBe("Reconnect");
		expect(state.action).toBe("reconnect");
	});

	test("keeps a queued refresh visible", () => {
		const state = syncConfidenceState(outcome("queued"), false, false);
		expect(state.label).toBe("Refreshing");
		expect(state.action).toBeNull();
	});

	test("returns the persisted failure and retry action", () => {
		const state = syncConfidenceState(
			outcome("failed", { error: "Provider refresh timed out" }),
			false,
			false,
		);
		expect(state.label).toBe("Needs attention");
		expect(state.description).toBe("Provider refresh timed out");
		expect(state.action).toBe("retry");
	});

	test("uses a stable absolute timestamp", () => {
		expect(formatSyncTimestamp("not-a-date")).toBe("Unknown");
		expect(formatSyncTimestamp(null)).toBe("Never");
		expect(formatSyncTimestamp("2026-08-13T10:00:10Z")).not.toBe("Unknown");
	});
});
