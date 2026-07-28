"use client";

import {
  Add01Icon,
  ArrowReloadHorizontalIcon,
  BankIcon,
  Delete02Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { toast } from "sonner";
import type { Account } from "@/components/accounts";
import { useActiveAccount } from "@/components/accounts";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import {
  useBrokerageBalances,
  useDisconnectBrokerage,
  useInitiateConnection,
  useSyncBrokerageData,
} from "@/hooks/brokerage";

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

function ConnectionCard({ account }: { account: Account }) {
  const { data: balances, isLoading } = useBrokerageBalances(account.id);
  const disconnect = useDisconnectBrokerage();
  const sync = useSyncBrokerageData();
  const initiate = useInitiateConnection();
  const [reconnecting, setReconnecting] = useState(false);

  async function handleReconnect() {
    setReconnecting(true);
    try {
      const callbackUrl = `${window.location.origin}/dashboard/brokerage/callback?accountId=${account.id}`;
      const portal = await initiate.mutateAsync({
        accountId: account.id,
        customRedirect: callbackUrl,
        reconnect: true,
      });
      window.location.href = portal.redirectUrl;
    } catch (err) {
      toast.error(
        `Failed to reconnect: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
      setReconnecting(false);
    }
  }

  async function handleSync() {
    try {
      const result = await sync.mutateAsync(account.id);
      toast.success(
        `Synced ${result.transactionsSynced} transactions, ${result.holdingsSynced} holdings`,
      );
    } catch (err) {
      toast.error(err instanceof Error ? err.message : "Failed to sync");
    }
  }

  async function handleDisconnect() {
    if (!confirm("Disconnect this brokerage? You can reconnect later.")) return;
    try {
      await disconnect.mutateAsync(account.id);
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
              {account.broker ?? "Brokerage"}
            </p>
            <p className="text-[0.65rem] text-muted-foreground">
              {account.name}
            </p>
          </div>
        </div>
        <div className="flex items-center gap-1">
          {account.snaptradeConnectionDisabled && (
            <>
              <span className="rounded bg-destructive/10 px-1.5 py-0.5 text-[0.6rem] font-medium text-destructive">
                Disconnected
              </span>
              <Button
                variant="outline"
                size="sm"
                onClick={handleReconnect}
                disabled={reconnecting}
                title="Reconnect"
              >
                {reconnecting ? "..." : "Reconnect"}
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
  const [open, setOpen] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const account = useActiveAccount();
  const connected = !!account?.snaptradeConnectionId;
  const initiate = useInitiateConnection();

  async function handleConnect() {
    if (!account) return;
    setConnecting(true);
    try {
      const callbackUrl = `${window.location.origin}/dashboard/brokerage/callback?accountId=${account.id}`;
      const portal = await initiate.mutateAsync({
        accountId: account.id,
        customRedirect: callbackUrl,
      });
      window.location.href = portal.redirectUrl;
    } catch (err) {
      toast.error(
        `Failed to connect: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
      setConnecting(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button variant="outline" size="sm">
          <HugeiconsIcon icon={BankIcon} strokeWidth={2} className="size-4" />
          Brokerage
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Brokerage Connections</DialogTitle>
          <DialogDescription>
            Connect your brokerages — Webull, Robinhood, Questrade, and more.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-3">
          {!connected ? (
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
                disabled={connecting || !account}
              >
                {connecting ? "Connecting..." : "Connect Brokerage"}
              </Button>
            </div>
          ) : (
            <>
              <ConnectionCard account={account!} />
              <Button
                variant="outline"
                size="sm"
                className="w-full"
                onClick={handleConnect}
                disabled={connecting}
              >
                <HugeiconsIcon
                  icon={Add01Icon}
                  strokeWidth={2}
                  className="size-4"
                />
                {connecting ? "Connecting..." : "Add Another Connection"}
              </Button>
            </>
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
}
