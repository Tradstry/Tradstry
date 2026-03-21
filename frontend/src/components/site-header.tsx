"use client"

import { usePathname } from "next/navigation"
import { Button } from "@/components/ui/button"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { HugeiconsIcon } from "@hugeicons/react"
import { AiChat02Icon } from "@hugeicons/core-free-icons"
import { useChatStore } from "@/hooks/chat"

const ROUTE_TITLES: Record<string, string> = {
  "/dashboard": "Dashboard",
  "/dashboard/playbook": "Playbook",
  "/dashboard/journal": "Journal",
  "/dashboard/notebook": "Notebook",
}

export function SiteHeader({ actions }: { actions?: React.ReactNode }) {
  const pathname = usePathname()
  const title = ROUTE_TITLES[pathname] ?? "Dashboard"
  const toggleOpen = useChatStore((s) => s.toggleOpen)

  return (
    <header className="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 data-[orientation=vertical]:h-4"
        />
        <h1 className="text-base font-medium">{title}</h1>
        {actions ? <div className="ml-4">{actions}</div> : null}
        <div className="ml-auto">
          <Button variant="outline" size="sm" onClick={toggleOpen}>
            <HugeiconsIcon icon={AiChat02Icon} strokeWidth={2} className="size-4" />
            Chat AI
          </Button>
        </div>
      </div>
    </header>
  )
}
