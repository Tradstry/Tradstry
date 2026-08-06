"use client";

import { BrokerageButton } from "@tradstry/app-ui/components/brokerage";
import { ChatButton } from "@tradstry/app-ui/components/chat";
import { getDashboardRouteMeta } from "@tradstry/app-ui/components/dashboard-route-meta";
import { NotificationsButton } from "@tradstry/app-ui/components/notifications";
import { WorkspaceSwitcher } from "@tradstry/app-ui/components/workspaces";
import { useTradstryPlatform } from "@tradstry/app-ui/platform";

export function SiteHeader({ actions }: { actions?: React.ReactNode }) {
  const { pathname } = useTradstryPlatform();
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
