"use client";

import {
	Alert02Icon,
	CheckmarkCircle02Icon,
	Loading03Icon,
	SecurityCheckIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@tradstry/app-ui/components/ui/dialog";
import {
	Select,
	SelectContent,
	SelectItem,
	SelectTrigger,
	SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { useReportBrokerageDataIssue } from "@tradstry/app-ui/hooks/brokerage";
import type {
	BrokerageDataIssueCategory,
	BrokerageDataIssueReport,
} from "@tradstry/app-ui/lib/types/brokerage";
import { useId, useState } from "react";

const ISSUE_OPTIONS: Array<{
	value: BrokerageDataIssueCategory;
	label: string;
}> = [
	{ value: "transactions", label: "Trades or fills" },
	{ value: "holdings", label: "Positions or holdings" },
	{ value: "balances", label: "Cash or buying power" },
	{ value: "account", label: "Wrong brokerage account" },
	{ value: "other", label: "Something else" },
];

interface ReportIncorrectDataDialogProps {
	workspaceId: string;
	workspaceName: string;
	brokerageAccountName: string;
	diagnosticId: string | null | undefined;
}

export function ReportIncorrectDataDialog({
	workspaceId,
	workspaceName,
	brokerageAccountName,
	diagnosticId,
}: ReportIncorrectDataDialogProps) {
	const report = useReportBrokerageDataIssue();
	const [open, setOpen] = useState(false);
	const [category, setCategory] =
		useState<BrokerageDataIssueCategory>("transactions");
	const [note, setNote] = useState("");
	const [result, setResult] = useState<BrokerageDataIssueReport | null>(null);
	const [error, setError] = useState<string | null>(null);
	const categoryId = useId();
	const noteId = useId();

	function handleOpenChange(nextOpen: boolean) {
		setOpen(nextOpen);
		if (!nextOpen) {
			setCategory("transactions");
			setNote("");
			setResult(null);
			setError(null);
			report.reset();
		}
	}

	async function handleSubmit() {
		setError(null);
		try {
			const created = await report.mutateAsync({
				workspaceId,
				category,
				note: note.trim() || undefined,
			});
			setResult(created);
		} catch (caught) {
			setError(
				caught instanceof Error
					? caught.message
					: "The report could not be sent. Try again.",
			);
		}
	}

	const requiresNote = category === "other";
	const canSubmit =
		!report.isPending && (!requiresNote || note.trim().length > 0);

	return (
		<Dialog open={open} onOpenChange={handleOpenChange}>
			<DialogTrigger asChild>
				<Button
					type="button"
					variant="ghost"
					size="sm"
					className="h-7 px-2 text-[0.625rem] text-muted-foreground hover:text-foreground"
				>
					<HugeiconsIcon
						icon={Alert02Icon}
						className="size-3.5"
						strokeWidth={2}
					/>
					Report incorrect data
				</Button>
			</DialogTrigger>
			<DialogContent className="sm:max-w-md">
				{result ? (
					<div className="py-2">
						<div className="flex size-9 items-center justify-center rounded-full bg-emerald-500/10 text-emerald-700 dark:text-emerald-400">
							<HugeiconsIcon
								icon={CheckmarkCircle02Icon}
								className="size-5"
								strokeWidth={2}
							/>
						</div>
						<h2 className="mt-3 text-sm font-semibold">Report received</h2>
						<p className="mt-1 text-xs leading-relaxed text-muted-foreground">
							The brokerage record has not been changed. This report gives the
							review team the evidence needed to investigate it.
						</p>
						<div className="mt-4 border-l-2 border-sky-500 bg-muted/30 px-3 py-2.5 font-mono text-[0.625rem]">
							<div className="flex justify-between gap-3">
								<span className="font-sans text-muted-foreground">
									Report ID
								</span>
								<span className="truncate" title={result.id}>
									{result.id}
								</span>
							</div>
							<div className="mt-1.5 flex justify-between gap-3">
								<span className="font-sans text-muted-foreground">
									Evidence ID
								</span>
								<span className="truncate" title={result.diagnosticId}>
									{result.diagnosticId}
								</span>
							</div>
						</div>
						<DialogFooter className="mt-4">
							<Button type="button" onClick={() => handleOpenChange(false)}>
								Done
							</Button>
						</DialogFooter>
					</div>
				) : (
					<>
						<DialogHeader>
							<DialogTitle>Report incorrect brokerage data</DialogTitle>
							<DialogDescription>
								Tell us what looks wrong. Tradstry will attach the current sync
								status and reconciliation counts.
							</DialogDescription>
						</DialogHeader>

						<div className="border-l-2 border-sky-500 bg-muted/30 px-3 py-2.5 text-[0.625rem]">
							<div className="flex items-center justify-between gap-3">
								<span className="text-muted-foreground">Account</span>
								<span
									className="truncate font-medium"
									title={brokerageAccountName}
								>
									{brokerageAccountName}
								</span>
							</div>
							<div className="mt-1.5 flex items-center justify-between gap-3">
								<span className="text-muted-foreground">Workspace</span>
								<span className="truncate font-medium" title={workspaceName}>
									{workspaceName}
								</span>
							</div>
							<div className="mt-1.5 flex items-center justify-between gap-3">
								<span className="text-muted-foreground">Evidence ID</span>
								<span
									className="max-w-[14rem] truncate font-mono"
									title={diagnosticId ?? undefined}
								>
									{diagnosticId ?? "Created when sent"}
								</span>
							</div>
						</div>

						<div>
							<label htmlFor={categoryId} className="font-medium">
								What looks wrong?
							</label>
							<Select
								value={category}
								onValueChange={(value) =>
									setCategory(value as BrokerageDataIssueCategory)
								}
							>
								<SelectTrigger id={categoryId} className="mt-1.5 w-full">
									<SelectValue />
								</SelectTrigger>
								<SelectContent>
									{ISSUE_OPTIONS.map((option) => (
										<SelectItem key={option.value} value={option.value}>
											{option.label}
										</SelectItem>
									))}
								</SelectContent>
							</Select>
						</div>

						<div>
							<div className="flex items-center justify-between gap-3">
								<label htmlFor={noteId} className="font-medium">
									What did you expect to see?{requiresNote ? "" : " (optional)"}
								</label>
								<span className="text-[0.6rem] tabular-nums text-muted-foreground">
									{note.length}/1000
								</span>
							</div>
							<textarea
								id={noteId}
								value={note}
								onChange={(event) => setNote(event.target.value)}
								maxLength={1000}
								rows={4}
								placeholder="For example: Webull shows one more fill on Aug 12."
								className="mt-1.5 w-full resize-none rounded-md border border-input bg-input/20 px-2.5 py-2 text-xs leading-relaxed outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30"
							/>
						</div>

						<div className="flex items-start gap-2 rounded-md border bg-muted/20 px-2.5 py-2 text-[0.625rem] leading-relaxed text-muted-foreground">
							<HugeiconsIcon
								icon={SecurityCheckIcon}
								className="mt-0.5 size-3.5 shrink-0"
								strokeWidth={2}
							/>
							<span>
								Credentials and raw brokerage payloads are never included.
								Sending this report does not edit your broker records.
							</span>
						</div>

						{error && (
							<p role="alert" className="text-[0.65rem] text-destructive">
								{error}
							</p>
						)}

						<DialogFooter>
							<Button
								type="button"
								variant="outline"
								onClick={() => handleOpenChange(false)}
							>
								Cancel
							</Button>
							<Button
								type="button"
								onClick={handleSubmit}
								disabled={!canSubmit}
							>
								{report.isPending && (
									<HugeiconsIcon
										icon={Loading03Icon}
										className="size-3.5 animate-spin"
										strokeWidth={2}
									/>
								)}
								{report.isPending ? "Sending…" : "Send report"}
							</Button>
						</DialogFooter>
					</>
				)}
			</DialogContent>
		</Dialog>
	);
}
