import type { BrokerageReconciliation } from "@tradstry/app-ui/lib/types/brokerage";

export type ReconciliationTone =
	| "neutral"
	| "progress"
	| "success"
	| "warning"
	| "danger";

export interface ReconciliationPresentation {
	label:
		| "Not verified"
		| "Verifying"
		| "Broker data verified"
		| "Data needs review"
		| "Portfolio unavailable"
		| "Verification failed";
	tone: ReconciliationTone;
	description: string;
	verified: boolean;
}

export function brokerageReconciliationPresentation(
	reconciliation: BrokerageReconciliation | null | undefined,
): ReconciliationPresentation {
	if (!reconciliation) {
		return {
			label: "Not verified",
			tone: "neutral",
			description: "Run a sync to compare the broker record with Tradstry.",
			verified: false,
		};
	}

	const statuses = [
		reconciliation.transactionStatus,
		reconciliation.portfolioStatus,
	];
	if (statuses.includes("failed")) {
		return {
			label: "Verification failed",
			tone: "danger",
			description:
				reconciliation.transactionError ??
				reconciliation.portfolioError ??
				"Tradstry could not complete the broker comparison.",
			verified: false,
		};
	}
	if (statuses.includes("discrepancy")) {
		return {
			label: "Data needs review",
			tone: "warning",
			description:
				"The broker record and the saved workspace data do not fully match.",
			verified: false,
		};
	}
	if (statuses.includes("pending")) {
		return {
			label: "Verifying",
			tone: "progress",
			description: "The brokerage is still preparing data for comparison.",
			verified: false,
		};
	}
	if (statuses.includes("unavailable")) {
		return {
			label: "Portfolio unavailable",
			tone: "warning",
			description:
				reconciliation.portfolioError ??
				"The broker did not provide a complete portfolio snapshot.",
			verified: false,
		};
	}
	if (statuses.every((status) => status === "matched")) {
		return {
			label: "Broker data verified",
			tone: "success",
			description: "Broker fills, holdings, and balances match this workspace.",
			verified: true,
		};
	}

	return {
		label: "Not verified",
		tone: "neutral",
		description: "Run a sync to complete the broker comparison.",
		verified: false,
	};
}
