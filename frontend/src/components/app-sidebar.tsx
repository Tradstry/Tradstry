"use client";

import {
  ComputerTerminalIcon,
  RoboticIcon,
  Settings05Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type * as React from "react";
import { AccountSwitcher } from "@/components/accounts";
import { NavMain } from "@/components/nav-main";
import { NavUser } from "@/components/nav-user";
import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarHeader,
  SidebarRail,
} from "@/components/ui/sidebar";

// This is sample data.
const data = {
  navMain: [
    {
      title: "Home",
      url: "/dashboard",
      icon: <HugeiconsIcon icon={ComputerTerminalIcon} strokeWidth={2} />,
      items: [
        { title: "Dashboard", url: "/dashboard" },
        { title: "Journal", url: "/dashboard/journal" },
        { title: "Notebook", url: "/dashboard/notebook" },
      ],
    },
    {
      title: "Analytics",
      url: "/dashboard/playbook",
      icon: <HugeiconsIcon icon={RoboticIcon} strokeWidth={2} />,
      items: [
        { title: "Playbook", url: "/dashboard/playbook" },
        { title: "Statistics", url: "/dashboard/statistics" },
        { title: "Reporting", url: "/dashboard/reporting" },
      ],
    },
    {
      title: "AI stuff",
      url: "/dashboard/ai-reports",
      icon: <HugeiconsIcon icon={RoboticIcon} strokeWidth={2} />,
      items: [
        { title: "AI Reports", url: "/dashboard/ai-reports" },
        { title: "AI Insights", url: "/dashboard/ai-insights" },
      ],
    },
    {
      title: "Resources",
      url: "/dashboard/mindset-lab",
      icon: <HugeiconsIcon icon={Settings05Icon} strokeWidth={2} />,
      items: [
        { title: "Mindset Lab", url: "/dashboard/mindset-lab" },
        { title: "Markets", url: "/dashboard/markets" },
        { title: "Charting", url: "/dashboard/charting" },
      ],
    },
  ],
};

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <AccountSwitcher />
      </SidebarHeader>
      <SidebarContent>
        <NavMain items={data.navMain} />
      </SidebarContent>
      <SidebarFooter>
        <NavUser />
      </SidebarFooter>
      <SidebarRail />
    </Sidebar>
  );
}
