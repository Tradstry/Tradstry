"use client";

import { BankIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@tradstry/app-ui/components/ui/empty";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import { useInitiateConnection } from "@tradstry/app-ui/hooks/brokerage";
import { capture, EVENTS } from "@tradstry/app-ui/lib/analytics/events";
import { platformUrl, useTradstryPlatform } from "@tradstry/app-ui/platform";

export function BrokerageEmptyState() {
  const workspace = useActiveWorkspace();
  const initiate = useInitiateConnection();
  const [connecting, setConnecting] = useState(false);
  const platform = useTradstryPlatform();

  async function handleConnect() {
    if (!workspace) return;

    setConnecting(true);
    capture(EVENTS.brokerageConnectStarted, {});

    try {
      // Build callback URL with workspaceId so the callback page knows which workspace to update
      const callbackUrl = platformUrl(
        platform,
        `/dashboard/brokerage/callback?workspaceId=${workspace.id}`,
      );

      const portal = await initiate.mutateAsync({
        workspaceId: workspace.id,
        customRedirect: callbackUrl,
      });

      // Redirect the user to the SnapTrade connection portal
      await platform.openExternal(portal.redirectUrl);
    } catch (err) {
      toast.error(
        `Failed to connect: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
      setConnecting(false);
    }
  }

  return (
    <div className="flex flex-1 items-center justify-center p-6">
      <Empty className="max-w-sm border-none">
        <EmptyHeader>
          <EmptyMedia variant="icon">
            <HugeiconsIcon icon={BankIcon} strokeWidth={2} />
          </EmptyMedia>
          <EmptyTitle>Connect your brokerage</EmptyTitle>
          <EmptyDescription>
            Link one brokerage account to this workspace to automatically sync your transaction
            history, positions, and balances.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button size="sm" onClick={handleConnect} disabled={connecting}>
            {connecting ? "Connecting..." : "Connect brokerage account"}
          </Button>
          <p className="text-xs text-muted-foreground">
            Supports 20+ brokerages via SnapTrade
          </p>
        </EmptyContent>
      </Empty>
    </div>
  );
}
