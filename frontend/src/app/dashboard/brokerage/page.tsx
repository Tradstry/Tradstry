"use client"

import { AppSidebar } from "@/components/app-sidebar"
import { useActiveAccount } from "@/components/accounts"
import { BrokerageEmptyState } from "@/components/brokerage/brokerage-empty-state"
import { BrokerageTransactions } from "@/components/brokerage/brokerage-transactions"
import { SiteHeader } from "@/components/site-header"
import { GraphQLProvider } from "@/lib/client"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { ChatProvider } from "@/components/chat/chat-panel"

function BrokerageContent() {
  const account = useActiveAccount()
  const isLinked = !!account?.snaptradeConnectionId

  if (!account) return null

  return isLinked ? <BrokerageTransactions /> : <BrokerageEmptyState />
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
