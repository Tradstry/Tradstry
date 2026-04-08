"use client"

import * as React from "react"

import { NavMain } from "@/components/nav-main"
import { NavUser } from "@/components/nav-user"
import { PositionCalculator } from "@/components/position-calculator"
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
} from "@/components/ui/sidebar"
import { HugeiconsIcon } from "@hugeicons/react"
import { DashboardSquare01Icon, File01Icon, CommandIcon, BookOpen01Icon, Notebook01Icon, BankIcon, Calculator01Icon } from "@hugeicons/core-free-icons"

const data = {
  navMain: [
    {
      title: "Dashboard",
      url: "/dashboard",
      icon: (
        <HugeiconsIcon icon={DashboardSquare01Icon} strokeWidth={2} />
      ),
    },
    {
      title: "Playbook",
      url: "/dashboard/playbook",
      icon: (
        <HugeiconsIcon icon={BookOpen01Icon} strokeWidth={2} />
      ),
    },
    {
      title: "Journal",
      url: "/dashboard/journal",
      icon: (
        <HugeiconsIcon icon={File01Icon} strokeWidth={2} />
      ),
    },
    {
      title: "Notebook",
      url: "/dashboard/notebook",
      icon: (
        <HugeiconsIcon icon={Notebook01Icon} strokeWidth={2} />
      ),
    },
    {
      title: "Brokerage",
      url: "/dashboard/brokerage",
      icon: (
        <HugeiconsIcon icon={BankIcon} strokeWidth={2} />
      ),
    },
  ],
}

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  const [calculatorOpen, setCalculatorOpen] = React.useState(false)

  return (
    <Sidebar collapsible="offcanvas" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton
              asChild
              className="data-[slot=sidebar-menu-button]:p-1.5!"
            >
              <a href="#">
                <HugeiconsIcon icon={CommandIcon} strokeWidth={2} className="size-5!" />
                <span className="text-base font-semibold">Tradstry</span>
              </a>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={data.navMain} />
      </SidebarContent>
      <SidebarFooter>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton onClick={() => setCalculatorOpen(true)}>
              <HugeiconsIcon icon={Calculator01Icon} strokeWidth={2} />
              <span>Position Calculator</span>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
        <NavUser />
      </SidebarFooter>
      <PositionCalculator open={calculatorOpen} onOpenChange={setCalculatorOpen} />
    </Sidebar>
  )
}
