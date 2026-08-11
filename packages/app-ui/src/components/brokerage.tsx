"use client";

import {
  ArrowReloadHorizontalIcon,
  BankIcon,
  Delete02Icon,
  Loading03Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "@tradstry/app-ui/components/ui/button";
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
  useDisconnectBrokerage,
  useInitiateConnection,
  useSyncBrokerageData,
} from "@tradstry/app-ui/hooks/brokerage";
import { platformUrl, useTradstryPlatform } from "@tradstry/app-ui/platform";
import { useState } from "react";
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

// ---------------------------------------------------------------------------
// ConnectionCard — one connected brokerage
// ---------------------------------------------------------------------------

function ConnectionCard({ workspace }: { workspace: Workspace }) {
  const platform = useTradstryPlatform();
  const { data: balances, isLoading } = useBrokerageBalances(workspace.id);
  const disconnect = useDisconnectBrokerage();
  const sync = useSyncBrokerageData();
  const initiate = useInitiateConnection();
  const [reconnecting, setReconnecting] = useState(false);

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
    try {
      const result = await sync.mutateAsync(workspace.id);
      if (result.status === "queued") {
        toast.info(
          "Brokerage refresh queued. Updated data will appear when available.",
        );
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
                disabled={sync.isPending}
                title="Sync"
              >
                <HugeiconsIcon
                  icon={ArrowReloadHorizontalIcon}
                  strokeWidth={2}
                  className={`size-3.5 ${sync.isPending ? "animate-spin" : ""}`}
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
        <div className="mt-2.5 flex flex-wrap gap-x-6 gap-y-1.5 border-t pt-2.5">
          {balances.map((b) => (
            <div key={b.id} className="flex items-baseline gap-1.5">
              <span className="text-[0.6rem] font-medium uppercase text-muted-foreground">
                {b.currency}
              </span>
              <span className="text-xs font-semibold tabular-nums">
                {formatCurrency(b.cash, b.currency)}
              </span>
            </div>
          ))}
        </div>
      ) : null}
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
