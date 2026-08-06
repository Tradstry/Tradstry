"use client";

import { usePathname } from "next/navigation";
import { BrokerageButton } from "@/components/brokerage";
import { ChatButton } from "@/components/chat";
import { getDashboardRouteMeta } from "@/components/dashboard-route-meta";
import { NotificationsButton } from "@/components/notifications";
import { WorkspaceSwitcher } from "@/components/workspaces";

export function SiteHeader({ actions }: { actions?: React.ReactNode }) {
  const pathname = usePathname();
  const title = getDashboardRouteMeta(pathname).title;

  return (
    <header className="sticky top-0 z-30 flex h-(--header-height) shrink-0 items-center border-b border-border/60 bg-background/80 backdrop-blur">
      <div className="flex w-full min-w-0 items-center px-4 lg:px-6">
        <div className="flex min-w-0 items-center">
          <h1 className="shrink-0 text-sm font-semibold tracking-tight">
            {title}
          </h1>
          <span className="mx-3 h-4 w-px shrink-0 bg-border" aria-hidden />
          <WorkspaceSwitcher />
        </div>
        {actions ? <div className="ml-3">{actions}</div> : null}
        <div className="ml-auto flex shrink-0 items-center gap-0.5">
          <BrokerageButton />
          <NotificationsButton />
          <ChatButton />
        </div>
      </div>
    </header>
  );
}
