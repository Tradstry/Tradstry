"use client";

import { usePathname } from "next/navigation";
import { getDashboardRouteMeta } from "@/components/dashboard-route-meta";
import { BrokerageButton } from "@/components/brokerage";
import { ChatButton } from "@/components/chat";

export function SiteHeader({ actions }: { actions?: React.ReactNode }) {
  const pathname = usePathname();
  const title = getDashboardRouteMeta(pathname).title;

  return (
    <header className="sticky top-0 z-30 flex h-(--header-height) shrink-0 items-center border-b border-border/60 bg-background/80 backdrop-blur">
      <div className="flex w-full items-center gap-2 px-4 lg:px-6">
        <h1 className="text-base font-semibold tracking-tight">{title}</h1>
        {actions ? <div className="ml-4">{actions}</div> : null}
        <div className="ml-auto flex items-center gap-2">
          <BrokerageButton />
          <ChatButton />
        </div>
      </div>
    </header>
  );
}
