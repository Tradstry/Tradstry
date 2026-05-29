"use client";

import { Analytics } from "@/components/analytics";
import { AppSidebar } from "@/components/app-sidebar";
import { ChatProvider } from "@/components/chat/chat-panel";
import { SiteHeader } from "@/components/site-header";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { GraphQLProvider } from "@/lib/client";

export default function AnalyticsPage() {
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
                <div className="flex flex-col gap-4 px-4 py-4 md:gap-6 md:px-6 md:py-6">
                  <Analytics />
                </div>
              </div>
            </div>
          </SidebarInset>
        </SidebarProvider>
      </ChatProvider>
    </GraphQLProvider>
  );
}
