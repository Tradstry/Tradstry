"use client";

import {
	ArrowReloadHorizontalIcon,
	BankIcon,
	Delete02Icon,
	Loading03Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { Checkbox } from "@tradstry/app-ui/components/ui/checkbox";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogHeader,
	DialogTitle,
	DialogTrigger,
} from "@tradstry/app-ui/components/ui/dialog";
import {
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from "@tradstry/app-ui/components/ui/tooltip";
import type { Workspace } from "@tradstry/app-ui/components/workspaces";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import {
	useBrokerageBalances,
	useBrokerageConnectionAccounts,
	useBrokerageSyncOutcome,
	useCreateBrokerageAccountWorkspaces,
	useDisconnectBrokerage,
	useInitiateConnection,
	useSyncBrokerageData,
} from "@tradstry/app-ui/hooks/brokerage";
import { platformUrl, useTradstryPlatform } from "@tradstry/app-ui/platform";
import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatCurrency(
	value: number | null | undefined,
	currency: string,
): string {
	if (value == null) return "—";
	try {
		return new Intl.NumberFormat("en-US", {
			style: "currency",
			currency,
			minimumFractionDigits: 2,
		}).format(value);
	} catch {
		return `${value.toFixed(2)} ${currency}`;
	}
}

function formatSyncTime(value: string): string {
	const date = new Date(value);
	if (Number.isNaN(date.getTime())) return "";
	return new Intl.DateTimeFormat("en-US", {
		hour: "numeric",
		minute: "2-digit",
	}).format(date);
}

function AdditionalBrokerageAccounts({ workspace }: { workspace: Workspace }) {
	const accounts = useBrokerageConnectionAccounts(
		workspace.id,
		!workspace.snaptradeConnectionDisabled,
	);
	const createWorkspaces = useCreateBrokerageAccountWorkspaces();
	const available = useMemo(
		() =>
			(accounts.data ?? []).filter(
				(account) => !account.current && !account.linkedWorkspaceId,
			),
		[accounts.data],
	);
	const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
	const initializedAccounts = useRef("");
	const availableKey = available.map((account) => account.id).join(":");

	useEffect(() => {
		if (initializedAccounts.current === availableKey) return;
		initializedAccounts.current = availableKey;
		setSelectedIds(new Set(available.map((account) => account.id)));
	}, [available, availableKey]);

	if (workspace.snaptradeConnectionDisabled) return null;
	if (accounts.isLoading) {
		return (
			<p className="mt-2.5 border-t pt-2.5 text-[0.65rem] text-muted-foreground">
				Checking for other brokerage accounts…
			</p>
		);
	}
	if (accounts.error) {
		return (
			<p className="mt-2.5 border-t pt-2.5 text-[0.65rem] text-destructive">
				Could not load the other accounts from this brokerage.
			</p>
		);
	}
	if (available.length === 0) return null;

	async function handleCreateWorkspaces() {
		try {
			const created = await createWorkspaces.mutateAsync({
				workspaceId: workspace.id,
				snaptradeAccountIds: [...selectedIds],
			});
			if (created.length === 0) {
				toast.info("The selected brokerage accounts are already linked");
				return;
			}
			toast.success(
				created.length === 1
					? `Created ${created[0]?.name ?? "brokerage"} workspace`
					: `Created ${created.length} brokerage workspaces`,
			);
		} catch (error) {
			toast.error(
				error instanceof Error
					? error.message
					: "Failed to create brokerage workspaces",
			);
		}
	}

	return (
		<div className="mt-2.5 border-t pt-2.5">
			<div className="space-y-0.5">
				<p className="text-xs font-medium">Other brokerage accounts</p>
				<p className="text-[0.65rem] text-muted-foreground">
					Create a separate workspace for each selected account.
				</p>
			</div>
			<div className="mt-2 grid gap-1.5">
				{available.map((account) => (
					<label
						key={account.id}
						htmlFor={`brokerage-account-${account.id}`}
						className="flex cursor-pointer items-center gap-2 rounded-md border px-2.5 py-2"
					>
						<Checkbox
							id={`brokerage-account-${account.id}`}
							checked={selectedIds.has(account.id)}
							onCheckedChange={(checked) => {
								setSelectedIds((current) => {
									const next = new Set(current);
									if (checked) next.add(account.id);
									else next.delete(account.id);
									return next;
								});
							}}
						/>
						<span className="min-w-0 flex-1">
							<span className="block truncate text-xs font-medium">
								{account.name}
							</span>
							{account.institutionName && (
								<span className="block truncate text-[0.625rem] text-muted-foreground">
									{account.institutionName}
								</span>
							)}
						</span>
					</label>
				))}
			</div>
			<Button
				className="mt-2.5 w-full"
				size="sm"
				onClick={handleCreateWorkspaces}
				disabled={selectedIds.size === 0 || createWorkspaces.isPending}
			>
				{createWorkspaces.isPending
					? "Creating workspaces…"
					: `Create ${selectedIds.size} workspace${selectedIds.size === 1 ? "" : "s"}`}
			</Button>
		</div>
	);
}

// ---------------------------------------------------------------------------
// ConnectionCard — one connected brokerage
// ---------------------------------------------------------------------------

function ConnectionCard({ workspace }: { workspace: Workspace }) {
	const platform = useTradstryPlatform();
	const [refreshQueued, setRefreshQueued] = useState(false);
	const refreshBaseline = useRef<string | null>(null);
	const outcomeBaseline = useRef<string | null>(null);
	const { data: balances, isLoading } = useBrokerageBalances(
		workspace.id,
		refreshQueued ? 5_000 : false,
	);
	const { data: syncOutcome } = useBrokerageSyncOutcome(
		workspace.id,
		refreshQueued ? 2_000 : false,
	);
	const disconnect = useDisconnectBrokerage();
	const sync = useSyncBrokerageData();
	const initiate = useInitiateConnection();
	const [reconnecting, setReconnecting] = useState(false);

	const latestBalanceSync = useMemo(() => {
		return (balances ?? []).reduce<string | null>((latest, balance) => {
			if (!balance.syncedAt) return latest;
			return latest === null || balance.syncedAt > latest
				? balance.syncedAt
				: latest;
		}, null);
	}, [balances]);

	useEffect(() => {
		if (!refreshQueued) return;
		if (
			syncOutcome?.status === "failed" &&
			syncOutcome.finishedAt !== outcomeBaseline.current
		) {
			setRefreshQueued(false);
			toast.error(syncOutcome.error ?? "Brokerage refresh failed");
			return;
		}
		if (
			syncOutcome?.status === "completed" &&
			syncOutcome.finishedAt !== outcomeBaseline.current
		) {
			setRefreshQueued(false);
			toast.success("Brokerage refresh complete");
			return;
		}
		if (latestBalanceSync && latestBalanceSync !== refreshBaseline.current) {
			setRefreshQueued(false);
			toast.success("Brokerage refresh complete");
			return;
		}
		const timeout = window.setTimeout(() => {
			setRefreshQueued(false);
			toast.info(
				"Brokerage is still refreshing. The latest saved data remains available.",
			);
		}, 90_000);
		return () => window.clearTimeout(timeout);
	}, [latestBalanceSync, refreshQueued, syncOutcome]);

	async function handleReconnect() {
		setReconnecting(true);
		try {
			const callbackUrl = platformUrl(
				platform,
				`/dashboard/brokerage/callback?workspaceId=${workspace.id}`,
			);
			const portal = await initiate.mutateAsync({
				workspaceId: workspace.id,
				customRedirect: callbackUrl,
				reconnect: true,
			});
			await platform.openExternal(portal.redirectUrl);
		} catch (err) {
			toast.error(
				`Failed to reconnect: ${err instanceof Error ? err.message : "Unknown error"}`,
			);
			setReconnecting(false);
		}
	}

	async function handleSync() {
		refreshBaseline.current = latestBalanceSync;
		outcomeBaseline.current = syncOutcome?.finishedAt ?? null;
		try {
			const result = await sync.mutateAsync(workspace.id);
			if (result.status === "queued") {
				setRefreshQueued(true);
				toast.info("Refreshing brokerage data. This can take up to a minute.");
			} else if (
				result.transactionsSynced === 0 &&
				result.holdingsSynced === 0 &&
				result.balancesSynced === 0
			) {
				toast.success("Brokerage data is already up to date");
			} else {
				toast.success(
					`Updated ${result.transactionsSynced} transactions, ${result.holdingsSynced} holdings, and ${result.balancesSynced} balances`,
				);
			}
		} catch (err) {
			setRefreshQueued(false);
			toast.error(err instanceof Error ? err.message : "Failed to sync");
		}
	}

	async function handleDisconnect() {
		if (!confirm("Disconnect this brokerage? You can reconnect later.")) return;
		try {
			await disconnect.mutateAsync(workspace.id);
			toast.success("Brokerage disconnected");
		} catch {
			toast.error("Failed to disconnect");
		}
	}

	return (
		<div className="rounded-lg border p-3">
			{/* Header row */}
			<div className="flex items-center justify-between">
				<div className="flex items-center gap-2.5">
					<div className="flex size-8 items-center justify-center rounded-md bg-emerald-50 text-emerald-600">
						<HugeiconsIcon icon={BankIcon} strokeWidth={2} className="size-4" />
					</div>
					<div>
						<p className="text-xs font-semibold">
							{workspace.broker ?? "Brokerage"}
						</p>
						<p className="text-[0.65rem] text-muted-foreground">
							{workspace.name}
						</p>
					</div>
				</div>
				<div className="flex items-center gap-1">
					{reconnecting ? (
						<output
							aria-label="Reconnecting brokerage"
							className="flex size-8 items-center justify-center text-muted-foreground"
						>
							<HugeiconsIcon
								icon={Loading03Icon}
								strokeWidth={2}
								className="size-4 animate-spin"
								aria-hidden
							/>
						</output>
					) : (
						<>
							{refreshQueued && (
								<span className="mr-1 text-[0.625rem] font-medium text-muted-foreground">
									Refreshing…
								</span>
							)}
							{workspace.snaptradeConnectionDisabled && (
								<>
									<span className="rounded bg-destructive/10 px-1.5 py-0.5 text-[0.6rem] font-medium text-destructive">
										Disconnected
									</span>
									<Button
										variant="outline"
										size="sm"
										onClick={handleReconnect}
										title="Reconnect"
									>
										Reconnect
									</Button>
								</>
							)}
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={handleSync}
								disabled={
									sync.isPending ||
									refreshQueued ||
									workspace.snaptradeConnectionDisabled
								}
								title={
									workspace.snaptradeConnectionDisabled
										? "Reconnect before syncing"
										: refreshQueued
											? "Refresh in progress"
											: "Sync"
								}
							>
								<HugeiconsIcon
									icon={ArrowReloadHorizontalIcon}
									strokeWidth={2}
									className={`size-3.5 ${sync.isPending || refreshQueued ? "animate-spin" : ""}`}
								/>
							</Button>
							<Button
								variant="ghost"
								size="icon-sm"
								onClick={handleDisconnect}
								disabled={disconnect.isPending}
								title="Disconnect"
								className="text-destructive hover:bg-destructive/10 hover:text-destructive"
							>
								<HugeiconsIcon
									icon={Delete02Icon}
									strokeWidth={2}
									className="size-3.5"
								/>
							</Button>
						</>
					)}
				</div>
			</div>

			{/* Balances */}
			{isLoading ? (
				<p className="mt-2 text-[0.65rem] text-muted-foreground">
					Loading balances...
				</p>
			) : balances && balances.length > 0 ? (
				<div className="mt-2.5 border-t pt-2.5">
					<div className="flex flex-wrap gap-x-6 gap-y-2">
						{balances.map((balance) => (
							<div key={balance.id} className="flex items-center gap-4">
								<span className="rounded-md bg-muted px-1.5 py-1 text-[0.6rem] font-semibold uppercase text-muted-foreground">
									{balance.currency}
								</span>
								<div>
									<p className="text-[0.6rem] text-muted-foreground">Cash</p>
									<p className="text-xs font-semibold tabular-nums">
										{formatCurrency(balance.cash, balance.currency)}
									</p>
								</div>
								<div>
									<p className="text-[0.6rem] text-muted-foreground">
										Buying power
									</p>
									<p className="text-xs font-semibold tabular-nums">
										{formatCurrency(balance.buyingPower, balance.currency)}
									</p>
								</div>
							</div>
						))}
					</div>
					{latestBalanceSync && (
						<p className="mt-2 text-[0.6rem] text-muted-foreground">
							Updated {formatSyncTime(latestBalanceSync)}
						</p>
					)}
				</div>
			) : null}
			<AdditionalBrokerageAccounts workspace={workspace} />
		</div>
	);
}

// ---------------------------------------------------------------------------
// BrokerageButton — header trigger + modal
// ---------------------------------------------------------------------------

export function BrokerageButton() {
	const platform = useTradstryPlatform();
	const [open, setOpen] = useState(false);
	const [connecting, setConnecting] = useState(false);
	const workspace = useActiveWorkspace();
	const connected = !!workspace?.snaptradeConnectionId;
	const initiate = useInitiateConnection();

	async function handleConnect() {
		if (!workspace) return;
		setConnecting(true);
		try {
			const callbackUrl = platformUrl(
				platform,
				`/dashboard/brokerage/callback?workspaceId=${workspace.id}`,
			);
			const portal = await initiate.mutateAsync({
				workspaceId: workspace.id,
				customRedirect: callbackUrl,
			});
			await platform.openExternal(portal.redirectUrl);
		} catch (err) {
			toast.error(
				`Failed to connect: ${err instanceof Error ? err.message : "Unknown error"}`,
			);
			setConnecting(false);
		}
	}

	return (
		<Dialog open={open} onOpenChange={setOpen}>
			<Tooltip>
				<TooltipTrigger asChild>
					<DialogTrigger asChild>
						<Button
							variant="ghost"
							size="icon"
							className="relative"
							aria-label="Brokerage"
						>
							<HugeiconsIcon
								icon={BankIcon}
								strokeWidth={2}
								className="size-4.5"
							/>
							{connected ? (
								<span
									className="absolute top-1 right-1 size-1.5 rounded-full bg-emerald-500 ring-2 ring-background"
									aria-hidden
								/>
							) : null}
						</Button>
					</DialogTrigger>
				</TooltipTrigger>
				<TooltipContent side="bottom">
					{connected ? "Brokerage connected" : "Connect brokerage"}
				</TooltipContent>
			</Tooltip>
			<DialogContent className="sm:max-w-md">
				<DialogHeader>
					<DialogTitle>Brokerage connection</DialogTitle>
					<DialogDescription>
						Connect one brokerage account to this workspace.
					</DialogDescription>
				</DialogHeader>

				<div className="flex flex-col gap-3">
					{!connected || !workspace ? (
						<div className="flex flex-col items-center gap-3 py-6 text-center">
							<div className="rounded-full bg-muted p-3">
								<HugeiconsIcon
									icon={BankIcon}
									strokeWidth={2}
									className="size-6 text-muted-foreground"
								/>
							</div>
							<div>
								<p className="text-sm font-medium">No connections yet</p>
								<p className="mt-1 text-xs text-muted-foreground">
									Link a brokerage to sync your trades, positions, and balances.
								</p>
							</div>
							<Button
								size="sm"
								onClick={handleConnect}
								disabled={connecting || !workspace}
							>
								{connecting ? (
									<>
										<HugeiconsIcon
											icon={Loading03Icon}
											strokeWidth={2}
											className="size-4 animate-spin"
											aria-hidden
										/>
										Connecting
									</>
								) : (
									"Connect Brokerage"
								)}
							</Button>
						</div>
					) : (
						<ConnectionCard workspace={workspace} />
					)}
				</div>
			</DialogContent>
		</Dialog>
	);
}
