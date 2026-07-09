"use client"

import { usePathname } from "next/navigation"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { BrokerageButton } from "@/components/brokerage"
import { ChatButton } from "@/components/chat"

const ROUTE_TITLES: Record<string, string> = {
  "/dashboard": "Dashboard",
  "/dashboard/playbook": "Playbook",
  "/dashboard/journal": "Journal",
  "/dashboard/notebook": "Notebook",
  "/dashboard/brokerage": "Brokerage",
}

export function SiteHeader({ actions }: { actions?: React.ReactNode }) {
  const pathname = usePathname()
  const title = ROUTE_TITLES[pathname] ?? "Dashboard"

  return (
    <header className="sticky top-0 z-30 flex h-(--header-height) shrink-0 items-center gap-2 border-b bg-background transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 data-[orientation=vertical]:h-4"
        />
        <h1 className="text-base font-medium">{title}</h1>
        {actions ? <div className="ml-4">{actions}</div> : null}
        <div className="ml-auto flex items-center gap-2">
          <BrokerageButton />
          <ChatButton />
        </div>
      </div>
    </header>
  )
}
