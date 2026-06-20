"use client"

import { useState } from "react"
import { AppSidebar } from "@/components/app-sidebar"
import { DashboardUpperCard, DashboardCalendar, DashboardRangeSelect } from "@/components/dashboard"
import { SiteHeader } from "@/components/site-header"
import { GraphQLProvider } from "@/lib/client"
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar"
import { ChatProvider } from "@/components/chat/chat-panel"
import type { AnalyticsRange } from "@/lib/types/analytics"

function DashboardInner() {
  const [range, setRange] = useState<AnalyticsRange>("LAST_1_MONTH")

  return (
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
          <SiteHeader
            actions={
              <DashboardRangeSelect value={range} onValueChange={setRange} />
            }
          />
          <div className="flex flex-1 flex-col overflow-auto">
            <div className="@container/main flex flex-1 flex-col gap-2">
              <div className="flex flex-col gap-4 px-4 py-4 md:gap-6 md:px-6 md:py-6">
                <DashboardUpperCard range={range} />
                <DashboardCalendar />
              </div>
            </div>
          </div>
        </SidebarInset>
      </SidebarProvider>
    </ChatProvider>
  )
}

export default function Page() {
  return (
    <GraphQLProvider>
      <DashboardInner />
    </GraphQLProvider>
  )
}
