import type { BrokerageSyncOutcome } from "@tradstry/app-ui/lib/types/brokerage";

export type SyncConfidenceTone = "neutral" | "progress" | "success" | "danger";

export interface SyncConfidenceState {
	label:
		| "Connected"
		| "Refreshing"
		| "Up to date"
		| "Needs attention"
		| "Reconnect";
	tone: SyncConfidenceTone;
	description: string;
	action: "sync" | "retry" | "reconnect" | null;
}

export function syncConfidenceState(
	outcome: BrokerageSyncOutcome | null | undefined,
	connectionDisabled: boolean,
	isRefreshing: boolean,
): SyncConfidenceState {
	if (connectionDisabled) {
		return {
			label: "Reconnect",
			tone: "danger",
			description:
				"Your brokerage authorization has expired. Reconnect it to resume syncing.",
			action: "reconnect",
		};
	}
	if (isRefreshing || outcome?.status === "queued") {
		return {
			label: "Refreshing",
			tone: "progress",
			description: "Tradstry is waiting for the brokerage refresh to finish.",
			action: null,
		};
	}
	if (outcome?.status === "failed") {
		return {
			label: "Needs attention",
			tone: "danger",
			description: outcome.error ?? "The latest brokerage sync did not finish.",
			action: "retry",
		};
	}
	if (outcome?.status === "completed") {
		return {
			label: "Up to date",
			tone: "success",
			description: "The latest brokerage sync completed successfully.",
			action: null,
		};
	}
	return {
		label: "Connected",
		tone: "neutral",
		description: "No brokerage sync has been recorded for this workspace yet.",
		action: "sync",
	};
}

export function formatSyncTimestamp(value: string | null | undefined): string {
	if (!value) return "Never";
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return "Unknown";
	return new Intl.DateTimeFormat("en-US", {
		dateStyle: "medium",
		timeStyle: "short",
	}).format(date);
}

export function formatNextSyncTimestamp(
	value: string | null | undefined,
	connectionDisabled = false,
): string {
	if (connectionDisabled) return "Reconnect to resume";
	if (!value) return "Not scheduled";
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return "Unknown";
	return new Intl.DateTimeFormat("en-US", {
		weekday: "short",
		month: "short",
		day: "numeric",
		hour: "numeric",
		minute: "2-digit",
	}).format(date);
}
