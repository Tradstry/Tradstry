"use client";

import { Button } from "@tradstry/app-ui/components/ui/button";
import {
	formatSyncTimestamp,
	syncConfidenceState,
} from "@tradstry/app-ui/lib/brokerage-sync-confidence";
import type { BrokerageSyncOutcome } from "@tradstry/app-ui/lib/types/brokerage";

const TONE_CLASSES = {
	neutral: "bg-muted text-muted-foreground",
	progress: "bg-amber-500/10 text-amber-700 dark:text-amber-400",
	success: "bg-emerald-500/10 text-emerald-700 dark:text-emerald-400",
	danger: "bg-destructive/10 text-destructive",
} as const;

interface SyncConfidenceCardProps {
	workspaceName: string;
	brokerageAccountName: string;
	outcome: BrokerageSyncOutcome | null | undefined;
	connectionDisabled: boolean;
	isRefreshing: boolean;
	isSyncing: boolean;
	isReconnecting: boolean;
	onSync: () => void;
	onReconnect: () => void;
}

export function SyncConfidenceCard({
	workspaceName,
	brokerageAccountName,
	outcome,
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

			<div className="mt-2 flex items-center justify-between gap-3 text-[0.625rem] text-muted-foreground">
				<span>Last successful sync</span>
				<span className="text-right font-medium text-foreground">
					{formatSyncTimestamp(outcome?.succeededAt)}
				</span>
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
