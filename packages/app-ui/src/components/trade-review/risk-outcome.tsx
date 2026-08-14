import type {
	StopBoundaryPosition,
	TradeReviewCalculation,
	TradeReviewUnavailableReason,
} from "@tradstry/app-ui/lib/types/position-calculator";
import { cn } from "@tradstry/app-ui/lib/utils";

function decimal(value: string | null | undefined, digits = 2) {
	if (value == null || value === "") return "—";
	const parsed = Number(value);
	return Number.isFinite(parsed)
		? parsed.toLocaleString("en-US", {
				minimumFractionDigits: digits,
				maximumFractionDigits: digits,
			})
		: value;
}

function money(value: string | null | undefined) {
	if (value == null || value === "") return "—";
	const parsed = Number(value);
	if (!Number.isFinite(parsed)) return `$${value}`;
	const sign = parsed < 0 ? "−" : "";
	return `${sign}$${decimal(String(Math.abs(parsed)))}`;
}

function multiple(value: string | null | undefined) {
	if (value == null || value === "") return "—";
	const parsed = Number(value);
	if (!Number.isFinite(parsed)) return `${value}R`;
	const sign = parsed > 0 ? "+" : parsed < 0 ? "−" : "";
	return `${sign}${decimal(String(Math.abs(parsed)))}R`;
}

const unavailableCopy: Record<TradeReviewUnavailableReason, string> = {
	invalid_planned_stop:
		"Realized R is unavailable because the saved plan has no valid stop boundary.",
	position_still_open:
		"Final stop outcome is unavailable until the broker position is closed.",
	incomplete_exit_quantity:
		"Final stop outcome is unavailable because the synced exits do not close the full entry quantity.",
	no_exit_fills:
		"Final stop outcome is unavailable because no broker exit fills were found.",
};

const boundaryCopy: Record<
	StopBoundaryPosition,
	{ label: string; className: string }
> = {
	before_planned_stop: {
		label: "Before stop",
		className:
			"border-emerald-500/30 bg-emerald-500/10 text-emerald-700 dark:text-emerald-300",
	},
	at_or_near_planned_stop: {
		label: "At or near stop",
		className:
			"border-amber-500/30 bg-amber-500/10 text-amber-700 dark:text-amber-300",
	},
	beyond_planned_stop: {
		label: "Beyond stop",
		className: "border-destructive/30 bg-destructive/10 text-destructive",
	},
};

function Metric({
	label,
	value,
	emphasis,
}: {
	label: string;
	value: string;
	emphasis?: "positive" | "negative";
}) {
	return (
		<div className="min-w-0">
			<p className="text-[0.625rem] font-semibold uppercase tracking-[0.07em] text-muted-foreground">
				{label}
			</p>
			<p
				className={cn(
					"mt-1 truncate font-mono text-sm font-semibold tabular-nums",
					emphasis === "positive" && "text-emerald-600 dark:text-emerald-400",
					emphasis === "negative" && "text-destructive",
				)}
			>
				{value}
			</p>
		</div>
	);
}

export function RiskOutcomePanel({
	calculation,
	loading = false,
}: {
	calculation: TradeReviewCalculation | null | undefined;
	loading?: boolean;
}) {
	if (loading) {
		return (
			<div className="rounded-lg border border-border bg-muted/20 px-3 py-4 text-xs text-muted-foreground">
				Calculating stop outcome from broker fills…
			</div>
		);
	}
	if (!calculation) return null;

	const realizedR = calculation.realized_r ?? calculation.planned_r_multiple;
	const realizedPnl = calculation.realized_pnl;
	const pnlNumber = realizedPnl == null ? null : Number(realizedPnl);
	const outcome = calculation.stop_outcome;
	const unavailableReason =
		calculation.stop_outcome_unavailable_reason ??
		calculation.realized_r_unavailable_reason;

	return (
		<section className="overflow-hidden rounded-lg border border-border bg-background">
			<div className="flex items-start justify-between gap-3 border-b border-border bg-muted/20 px-3 py-2.5">
				<div>
					<p className="text-xs font-semibold">Stop outcome & realized R</p>
					<p className="mt-0.5 text-[0.6875rem] text-muted-foreground">
						Planned risk compared with locked broker executions
					</p>
				</div>
				<span className="rounded-full border border-border bg-background px-2 py-1 text-[0.625rem] font-medium uppercase tracking-[0.06em] text-muted-foreground">
					Broker evidence
				</span>
			</div>

			<div className="grid grid-cols-2 gap-3 px-3 py-3 sm:grid-cols-4">
				<Metric label="Planned risk" value={money(calculation.planned_risk)} />
				<Metric
					label="Net result"
					value={money(realizedPnl)}
					emphasis={
						pnlNumber == null
							? undefined
							: pnlNumber >= 0
								? "positive"
								: "negative"
					}
				/>
				<Metric
					label="Realized R"
					value={multiple(realizedR)}
					emphasis={
						realizedR == null
							? undefined
							: Number(realizedR) >= 0
								? "positive"
								: "negative"
					}
				/>
				<Metric label="Broker fees" value={money(calculation.total_fees)} />
			</div>

			{outcome ? (
				<div className="border-t border-border px-3 py-3">
					<div className="grid grid-cols-[1fr_auto_1fr] items-center gap-2 text-[0.625rem] font-medium uppercase tracking-[0.06em] text-muted-foreground">
						<span className="text-destructive">Beyond boundary</span>
						<span className="rounded border border-border bg-muted/30 px-2 py-1 font-mono tabular-nums text-foreground">
							Stop ${decimal(outcome.planned_stop)}
						</span>
						<span className="text-right text-emerald-700 dark:text-emerald-300">
							Before boundary
						</span>
					</div>
					<div className="mt-2 h-1.5 rounded-full bg-gradient-to-r from-destructive/70 via-amber-400/70 to-emerald-500/70" />

					<div className="mt-3 grid gap-1.5">
						{outcome.exits.map((exit) => {
							const boundary = boundaryCopy[exit.boundary_position];
							return (
								<div
									key={`${exit.transaction_id}-${exit.executed_at}`}
									className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 rounded-md border border-border/70 bg-muted/15 px-2.5 py-2"
								>
									<div className="min-w-0">
										<p className="truncate font-mono text-xs tabular-nums">
											{decimal(exit.quantity, 4)} @ ${decimal(exit.price, 4)}
										</p>
										<p className="mt-0.5 text-[0.625rem] tabular-nums text-muted-foreground">
											{new Date(exit.executed_at).toLocaleString()} · fee{" "}
											{money(exit.fee)}
										</p>
									</div>
									<div className="text-right">
										<span
											className={cn(
												"inline-flex rounded-full border px-2 py-0.5 text-[0.625rem] font-medium",
												boundary.className,
											)}
										>
											{boundary.label}
										</span>
										<p className="mt-1 font-mono text-[0.625rem] tabular-nums text-muted-foreground">
											{multiple(exit.distance_from_stop_r)} from stop
										</p>
									</div>
								</div>
							);
						})}
					</div>
					<p className="mt-2 text-[0.625rem] text-muted-foreground">
						“Near” means within {multiple(outcome.near_tolerance_r)} of the
						planned boundary. Execution prices show where exits occurred; they
						do not prove that a stop order triggered.
					</p>
				</div>
			) : unavailableReason ? (
				<div className="border-t border-border bg-muted/15 px-3 py-3 text-xs text-muted-foreground">
					{unavailableCopy[unavailableReason]}
				</div>
			) : null}
		</section>
	);
}
