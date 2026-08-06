"use client";

import { AppSidebar } from "@/components/app-sidebar";
import { ChatProvider } from "@/components/chat/chat-panel";
import { Notebook } from "@/components/notebook";
import { SiteHeader } from "@/components/site-header";
import { SidebarInset, SidebarProvider } from "@/components/ui/sidebar";
import { GraphQLProvider } from "@/lib/client";

export default function NotebookPage() {
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
          <AppSidebar />
          <SidebarInset>
            <SiteHeader />
            {/* Full-bleed: the notebook owns its own three-column layout. */}
            <div className="flex min-h-0 flex-1">
              <Notebook />
            </div>
          </SidebarInset>
        </SidebarProvider>
      </ChatProvider>
    </GraphQLProvider>
  );
}
