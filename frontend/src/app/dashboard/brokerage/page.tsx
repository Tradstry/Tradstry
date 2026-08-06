"use client";

import { useState } from "react";
import { toast } from "sonner";
import { AppSidebar } from "@/components/app-sidebar";
import { BrokerageEmptyState } from "@/components/brokerage/brokerage-empty-state";
import { BrokerageTransactions } from "@/components/brokerage/brokerage-transactions";
import { ChatProvider } from "@/components/chat/chat-panel";
import { SiteHeader } from "@/components/site-header";
import { Button } from "@/components/ui/button";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { useActiveWorkspace } from "@/components/workspaces";
import { useInitiateConnection } from "@/hooks/brokerage";
import { GraphQLProvider } from "@/lib/client";

function DisconnectedBanner({
  workspaceId,
  broker,
  disabledAt,
}: {
  workspaceId: string;
  broker: string | null;
  disabledAt: string | null;
}) {
  const initiate = useInitiateConnection();
  const [connecting, setConnecting] = useState(false);

  async function handleReconnect() {
    setConnecting(true);
    try {
      const callbackUrl = `${window.location.origin}/dashboard/brokerage/callback?workspaceId=${workspaceId}`;
      const portal = await initiate.mutateAsync({
        workspaceId,
        customRedirect: callbackUrl,
        reconnect: true,
      });
      window.location.href = portal.redirectUrl;
    } catch (err) {
      toast.error(
        `Failed to reconnect: ${err instanceof Error ? err.message : "Unknown error"}`,
      );
      setConnecting(false);
    }
  }

  const when = disabledAt ? new Date(disabledAt).toLocaleDateString() : null;

  return (
    <div className="mx-4 mt-2 flex items-center justify-between gap-3 rounded-lg border border-amber-300 bg-amber-50 px-4 py-2.5 text-amber-900 dark:border-amber-700/50 dark:bg-amber-950/40 dark:text-amber-200">
      <p className="text-sm">
        Your {broker ?? "brokerage"} connection was disconnected
        {when ? ` on ${when}` : ""}. The data shown may be out of date.
      </p>
      <Button size="sm" onClick={handleReconnect} disabled={connecting}>
        {connecting ? "Reconnecting..." : "Reconnect"}
      </Button>
    </div>
  );
}

function BrokerageContent() {
  const workspace = useActiveWorkspace();
  const isLinked = !!workspace?.snaptradeConnectionId;

  if (!workspace) return null;
  if (!isLinked) return <BrokerageEmptyState />;

  return (
    <>
      {workspace.snaptradeConnectionDisabled && (
        <DisconnectedBanner
          workspaceId={workspace.id}
          broker={workspace.broker}
          disabledAt={workspace.snaptradeConnectionDisabledAt}
        />
      )}
      <BrokerageTransactions />
    </>
  );
}

export default function BrokeragePage() {
  return (
    <GraphQLProvider>
      <ChatProvider>
        <SidebarProvider
          style={
            {
              "--sidebar-width": "calc(var(--spacing) * 72)",
              "--header-height": "calc(var(--spacing) * 12)",
            } as React.CSSProperties
          }
        >
          <AppSidebar />
          <SidebarInset>
            <SiteHeader />
            <div className="flex flex-1 flex-col overflow-y-auto">
              <div className="@container/main flex flex-1 flex-col gap-2">
                <BrokerageContent />
              </div>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </ChatProvider>
    </GraphQLProvider>
  );
}
