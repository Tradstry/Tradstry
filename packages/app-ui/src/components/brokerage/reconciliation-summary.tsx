"use client";

import {
	Alert02Icon,
	CheckmarkCircle02Icon,
	InformationCircleIcon,
	Loading03Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
	brokerageReconciliationPresentation,
	type ReconciliationTone,
} from "@tradstry/app-ui/lib/brokerage-reconciliation";
import { formatSyncTimestamp } from "@tradstry/app-ui/lib/brokerage-sync-confidence";
import type { BrokerageReconciliation } from "@tradstry/app-ui/lib/types/brokerage";
import type { ReactNode } from "react";

const TONE_CLASSES: Record<ReconciliationTone, string> = {
	neutral: "border-border bg-muted/20 text-muted-foreground",
	progress:
		"border-amber-500/25 bg-amber-500/[0.06] text-amber-700 dark:text-amber-400",
	success:
		"border-emerald-500/25 bg-emerald-500/[0.06] text-emerald-700 dark:text-emerald-400",
	warning:
		"border-amber-500/25 bg-amber-500/[0.06] text-amber-700 dark:text-amber-400",
	danger: "border-destructive/25 bg-destructive/[0.06] text-destructive",
};

function StatusIcon({ tone }: { tone: ReconciliationTone }) {
	const icon =
		tone === "success"
			? CheckmarkCircle02Icon
			: tone === "danger" || tone === "warning"
				? Alert02Icon
				: tone === "progress"
					? Loading03Icon
					: InformationCircleIcon;
	return (
		<HugeiconsIcon
			icon={icon}
			className={`mt-0.5 size-3.5 shrink-0 ${tone === "progress" ? "animate-spin" : ""}`}
			strokeWidth={2}
		/>
	);
}

export function ReconciliationSummary({
	reconciliation,
}: {
	reconciliation: BrokerageReconciliation | null | undefined;
}) {
	const presentation = brokerageReconciliationPresentation(reconciliation);
	const brokerCount = reconciliation?.brokerTransactionCount;
	const localCount = reconciliation?.localTransactionCount;

	return (
		<section aria-label="Broker data verification" className="mt-3">
			<div
				className={`rounded-md border px-2.5 py-2 ${TONE_CLASSES[presentation.tone]}`}
			>
				<div className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
					<div className="flex min-w-0 items-start gap-2">
						<StatusIcon tone={presentation.tone} />
						<div className="min-w-0">
							<p className="text-[0.65rem] font-semibold text-foreground">
								{presentation.label}
							</p>
							<p className="mt-0.5 text-[0.6rem] leading-relaxed">
								{presentation.description}
							</p>
						</div>
					</div>
					<div className="flex shrink-0 items-center gap-2 rounded-md border border-current/15 bg-background/70 px-2 py-1 text-[0.6rem] tabular-nums text-foreground">
						<span>
							<span className="text-muted-foreground">Broker</span>{" "}
							<strong>{brokerCount ?? "—"}</strong>
						</span>
						<span aria-hidden="true" className="text-muted-foreground">
							⇄
						</span>
						<span>
							<span className="text-muted-foreground">Tradstry</span>{" "}
							<strong>{localCount ?? "—"}</strong>
						</span>
					</div>
				</div>
			</div>

			{reconciliation && (
				<details className="mt-2 text-[0.625rem]">
					<summary className="cursor-pointer select-none font-medium text-muted-foreground hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2">
						Verification details
					</summary>
					<div className="mt-2 grid gap-2 rounded-md border bg-muted/20 p-2.5 sm:grid-cols-2">
						<MetricGroup title="Fills">
							<Metric
								label="Imported this sync"
								value={reconciliation.importedTransactionCount}
							/>
							<Metric
								label="Already stored"
								value={reconciliation.duplicateTransactionCount}
							/>
							<Metric
								label="Skipped"
								value={reconciliation.skippedTransactionCount}
								attention={reconciliation.skippedTransactionCount > 0}
							/>
							<Metric
								label="Pending"
								value={reconciliation.pendingTransactionCount}
								attention={reconciliation.pendingTransactionCount > 0}
							/>
							<Metric
								label="Failed"
								value={reconciliation.failedTransactionCount}
								attention={reconciliation.failedTransactionCount > 0}
							/>
						</MetricGroup>
						<MetricGroup title="Differences">
							<Metric
								label="Missing broker fills"
								value={reconciliation.missingTransactionCount}
								attention={reconciliation.missingTransactionCount > 0}
							/>
							<Metric
								label="Local-only fills"
								value={reconciliation.extraTransactionCount}
								attention={reconciliation.extraTransactionCount > 0}
							/>
							<Metric
								label="Broker holdings"
								value={reconciliation.brokerHoldingCount}
							/>
							<Metric
								label="Saved holdings"
								value={reconciliation.localHoldingCount}
							/>
							<Metric
								label="Balance differences"
								value={reconciliation.balanceDiscrepancyCount}
								attention={reconciliation.balanceDiscrepancyCount > 0}
							/>
						</MetricGroup>
						<dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1.5 border-t pt-2 text-muted-foreground sm:col-span-2">
							<dt>Fills checked</dt>
							<dd className="text-right text-foreground">
								{formatSyncTimestamp(reconciliation.transactionCheckedAt)}
							</dd>
							<dt>Portfolio checked</dt>
							<dd className="text-right text-foreground">
								{formatSyncTimestamp(reconciliation.portfolioCheckedAt)}
							</dd>
							<dt>Verification ID</dt>
							<dd
								className="truncate text-right font-mono text-foreground"
								title={reconciliation.diagnosticId}
							>
								{reconciliation.diagnosticId}
							</dd>
						</dl>
					</div>
				</details>
			)}
		</section>
	);
}

function MetricGroup({
	title,
	children,
}: {
	title: string;
	children: ReactNode;
}) {
	return (
		<div>
			<p className="mb-1.5 font-semibold text-foreground">{title}</p>
			<dl className="space-y-1 text-muted-foreground">{children}</dl>
		</div>
	);
}

function Metric({
	label,
	value,
	attention = false,
}: {
	label: string;
	value: number;
	attention?: boolean;
}) {
	return (
		<div className="flex items-center justify-between gap-3">
			<dt>{label}</dt>
			<dd
				className={`font-medium tabular-nums ${attention ? "text-amber-700 dark:text-amber-400" : "text-foreground"}`}
			>
				{value}
			</dd>
		</div>
	);
}
