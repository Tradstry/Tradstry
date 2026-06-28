"use client"

import { useState } from "react"
import { AppSidebar } from "@/components/app-sidebar"
import { useActiveAccount } from "@/components/accounts"
import { BrokerageEmptyState } from "@/components/brokerage/brokerage-empty-state"
import { BrokerageTransactions } from "@/components/brokerage/brokerage-transactions"
import { SiteHeader } from "@/components/site-header"
import { Button } from "@/components/ui/button"
import { GraphQLProvider } from "@/lib/client"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { ChatProvider } from "@/components/chat/chat-panel"
import { useInitiateConnection } from "@/hooks/brokerage"
import { toast } from "sonner"

function DisconnectedBanner({
  accountId,
  broker,
  disabledAt,
}: {
  accountId: string
  broker: string | null
  disabledAt: string | null
}) {
  const initiate = useInitiateConnection()
  const [connecting, setConnecting] = useState(false)

  async function handleReconnect() {
    setConnecting(true)
    try {
      const callbackUrl = `${window.location.origin}/dashboard/brokerage/callback?accountId=${accountId}`
      const portal = await initiate.mutateAsync({
        accountId,
        customRedirect: callbackUrl,
        reconnect: true,
      })
      window.location.href = portal.redirectUrl
    } catch (err) {
      toast.error(
        `Failed to reconnect: ${err instanceof Error ? err.message : "Unknown error"}`,
      )
      setConnecting(false)
    }
  }

  const when = disabledAt ? new Date(disabledAt).toLocaleDateString() : null

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
  )
}

function BrokerageContent() {
  const account = useActiveAccount()
  const isLinked = !!account?.snaptradeConnectionId

  if (!account) return null
  if (!isLinked) return <BrokerageEmptyState />

  return (
    <>
      {account.snaptradeConnectionDisabled && (
        <DisconnectedBanner
          accountId={account.id}
          broker={account.broker}
          disabledAt={account.snaptradeConnectionDisabledAt}
        />
      )}
      <BrokerageTransactions />
    </>
  )
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
          <AppSidebar variant="inset" />
          <SidebarInset>
            <SiteHeader />
            <div className="flex flex-1 flex-col">
              <div className="@container/main flex flex-1 flex-col gap-2">
                <BrokerageContent />
              </div>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </ChatProvider>
    </GraphQLProvider>
  )
}
