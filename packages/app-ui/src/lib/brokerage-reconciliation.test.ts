import { describe, expect, test } from "bun:test";
import { brokerageReconciliationPresentation } from "./brokerage-reconciliation";
import type { BrokerageReconciliation } from "./types/brokerage";

function reconciliation(
	overrides: Partial<BrokerageReconciliation> = {},
): BrokerageReconciliation {
	return {
		diagnosticId: "diag-webull-cash",
		transactionStatus: "matched",
		transactionCheckedAt: "2026-08-14T08:00:00Z",
		brokerTransactionCount: 12,
		mappedTransactionCount: 12,
		importedTransactionCount: 2,
		duplicateTransactionCount: 10,
		skippedTransactionCount: 0,
		pendingTransactionCount: 0,
		failedTransactionCount: 0,
		localTransactionCount: 12,
		missingTransactionCount: 0,
		extraTransactionCount: 0,
		portfolioStatus: "matched",
		portfolioCheckedAt: "2026-08-14T08:00:01Z",
		brokerHoldingCount: 3,
		mappedHoldingCount: 3,
		localHoldingCount: 3,
		brokerBalanceCount: 1,
		localBalanceCount: 1,
		balanceDiscrepancyCount: 0,
		transactionError: null,
		portfolioError: null,
		...overrides,
	};
}

describe("brokerage reconciliation presentation", () => {
	test("only reports verified when transactions and portfolio both match", () => {
		const result = brokerageReconciliationPresentation(reconciliation());
		expect(result.label).toBe("Broker data verified");
		expect(result.verified).toBe(true);
	});

	test("prioritizes a persisted failure and its explanation", () => {
		const result = brokerageReconciliationPresentation(
			reconciliation({
				transactionStatus: "failed",
				transactionError: "Webull history request timed out",
			}),
		);
		expect(result.label).toBe("Verification failed");
		expect(result.description).toBe("Webull history request timed out");
	});

	test("distinguishes a discrepancy from an in-progress backfill", () => {
		expect(
			brokerageReconciliationPresentation(
				reconciliation({ transactionStatus: "discrepancy" }),
			).label,
		).toBe("Data needs review");
		expect(
			brokerageReconciliationPresentation(
				reconciliation({ transactionStatus: "pending" }),
			).label,
		).toBe("Verifying");
	});
});
