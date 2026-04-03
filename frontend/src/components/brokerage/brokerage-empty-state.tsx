"use client";

import { useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { BankIcon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { useActiveAccount } from "@/components/accounts";
import { useInitiateConnection } from "@/hooks/brokerage";
import { toast } from "sonner";

export function BrokerageEmptyState() {
  const account = useActiveAccount();
  const initiate = useInitiateConnection();
  const [connecting, setConnecting] = useState(false);

  async function handleConnect() {
    if (!account) return;

    setConnecting(true);

    try {
      // Build callback URL with accountId so the callback page knows which account to update
      const callbackUrl = `${window.location.origin}/dashboard/brokerage/callback?accountId=${account.id}`;

      const portal = await initiate.mutateAsync({
        accountId: account.id,
        customRedirect: callbackUrl,
      });

      // Redirect the user to the SnapTrade connection portal
      window.location.href = portal.redirectUrl;
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
            Link your brokerage account to automatically sync your transaction
            history, positions, and balances.
          </EmptyDescription>
        </EmptyHeader>
        <EmptyContent>
          <Button size="sm" onClick={handleConnect} disabled={connecting}>
            {connecting ? "Connecting..." : "Connect Account"}
          </Button>
          <p className="text-xs text-muted-foreground">
            Supports 20+ brokerages via SnapTrade
          </p>
        </EmptyContent>
      </Empty>
    </div>
  );
}
