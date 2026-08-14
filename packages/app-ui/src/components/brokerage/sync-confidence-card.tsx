"use client";

import { ReconciliationSummary } from "@tradstry/app-ui/components/brokerage/reconciliation-summary";
import { ReportIncorrectDataDialog } from "@tradstry/app-ui/components/brokerage/report-incorrect-data-dialog";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
	formatNextSyncTimestamp,
	formatSyncTimestamp,
	syncConfidenceState,
} from "@tradstry/app-ui/lib/brokerage-sync-confidence";
import type {
	BrokerageReconciliation,
	BrokerageSyncOutcome,
} from "@tradstry/app-ui/lib/types/brokerage";

const TONE_CLASSES = {
	neutral: "bg-muted text-muted-foreground",
	progress: "bg-amber-500/10 text-amber-700 dark:text-amber-400",
	success: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
	danger: "bg-destructive/10 text-destructive",
} as const;

interface SyncConfidenceCardProps {
	workspaceId: string;
	workspaceName: string;
	brokerageAccountName: string;
	outcome: BrokerageSyncOutcome | null | undefined;
	reconciliation: BrokerageReconciliation | null | undefined;
	connectionDisabled: boolean;
	isRefreshing: boolean;
	isSyncing: boolean;
	isReconnecting: boolean;
	onSync: () => void;
	onReconnect: () => void;
}

export function SyncConfidenceCard({
	workspaceId,
	workspaceName,
	brokerageAccountName,
	outcome,
	reconciliation,
	connectionDisabled,
	isRefreshing,
	isSyncing,
	isReconnecting,
	onSync,
	onReconnect,
}: SyncConfidenceCardProps) {
	const state = syncConfidenceState(outcome, connectionDisabled, isRefreshing);
	const showAction = state.action !== null;
	const actionIsReconnect = state.action === "reconnect";

	return (
		<section aria-label="Brokerage sync status" className="mt-3 border-t pt-3">
			<div className="flex items-start justify-between gap-3">
				<div className="min-w-0">
					<div className="flex items-center gap-2">
						<span
							className={`rounded-md px-2 py-1 text-[0.625rem] font-semibold ${TONE_CLASSES[state.tone]}`}
						>
							{state.label}
						</span>
						<p className="truncate text-xs font-medium">
							{brokerageAccountName}
						</p>
					</div>
					<p className="mt-1 text-[0.65rem] leading-relaxed text-muted-foreground">
						{state.description}
					</p>
				</div>
				{showAction && (
					<Button
						type="button"
						variant={actionIsReconnect ? "default" : "outline"}
						size="sm"
						onClick={actionIsReconnect ? onReconnect : onSync}
						disabled={isSyncing || isReconnecting}
					>
						{actionIsReconnect
							? isReconnecting
								? "Opening…"
								: "Reconnect"
							: isSyncing
								? "Syncing…"
								: state.action === "retry"
									? "Retry"
									: "Sync now"}
					</Button>
				)}
			</div>

			<div className="mt-3 grid grid-cols-3 divide-x rounded-md border bg-muted/20 py-2">
				<SyncCount
					label="Transactions"
					value={outcome?.transactionsSynced ?? 0}
				/>
				<SyncCount label="Holdings" value={outcome?.holdingsSynced ?? 0} />
				<SyncCount label="Balances" value={outcome?.balancesSynced ?? 0} />
			</div>

			<div className="mt-2 grid grid-cols-2 divide-x rounded-md border bg-background text-[0.625rem]">
				<div className="px-2.5 py-2">
					<p className="text-muted-foreground">Last successful</p>
					<p className="mt-0.5 font-medium text-foreground">
						{formatSyncTimestamp(outcome?.succeededAt)}
					</p>
				</div>
				<div className="px-2.5 py-2">
					<p className="text-muted-foreground">Next automatic sync</p>
					<p className="mt-0.5 font-medium text-foreground">
						{formatNextSyncTimestamp(
							outcome?.nextScheduledAt,
							connectionDisabled,
						)}
					</p>
				</div>
			</div>

			<ReconciliationSummary reconciliation={reconciliation} />

			<div className="mt-1 flex justify-end">
				<ReportIncorrectDataDialog
					workspaceId={workspaceId}
					workspaceName={workspaceName}
					brokerageAccountName={brokerageAccountName}
					diagnosticId={reconciliation?.diagnosticId ?? outcome?.diagnosticId}
				/>
			</div>

			<details className="mt-2 border-t pt-2 text-[0.625rem]">
				<summary className="cursor-pointer select-none font-medium text-muted-foreground hover:text-foreground">
					Sync details
				</summary>
				<dl className="mt-2 grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 rounded-md bg-muted/30 p-2.5 text-muted-foreground">
					<dt>Workspace</dt>
					<dd className="truncate text-right text-foreground">
						{workspaceName}
					</dd>
					<dt>Latest attempt</dt>
					<dd className="text-right text-foreground">
						{formatSyncTimestamp(outcome?.startedAt)}
					</dd>
					<dt>Finished</dt>
					<dd className="text-right text-foreground">
						{formatSyncTimestamp(outcome?.finishedAt)}
					</dd>
					<dt>Diagnostic ID</dt>
					<dd
						className="truncate text-right font-mono text-foreground"
						title={outcome?.diagnosticId ?? undefined}
					>
						{outcome?.diagnosticId ?? "Not available"}
					</dd>
				</dl>
			</details>
		</section>
	);
}

function SyncCount({ label, value }: { label: string; value: number }) {
	return (
		<div className="px-2 text-center">
			<p className="text-sm font-semibold tabular-nums">{value}</p>
			<p className="mt-0.5 text-[0.55rem] uppercase tracking-wide text-muted-foreground">
				{label}
			</p>
		</div>
	);
}
