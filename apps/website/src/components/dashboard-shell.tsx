"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import type * as React from "react";
import { AppSidebar } from "@/components/app-sidebar";
import { getDashboardRouteMeta } from "@/components/dashboard-route-meta";
import {
  Breadcrumb,
  BreadcrumbItem,
  BreadcrumbLink,
  BreadcrumbList,
  BreadcrumbPage,
  BreadcrumbSeparator,
} from "@/components/ui/breadcrumb";
import { Separator } from "@/components/ui/separator";
import {
  SidebarInset,
  SidebarProvider,
  SidebarTrigger,
} from "@/components/ui/sidebar";

export function DashboardShell({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();
  const meta = getDashboardRouteMeta(pathname);

  return (
    <SidebarProvider>
      <AppSidebar />
      <SidebarInset>
        <header className="sticky top-0 z-20 border-b bg-background/95 backdrop-blur supports-backdrop-filter:bg-background/80">
          <div className="flex h-14 items-center gap-2 px-4">
            <SidebarTrigger className="-ml-1" />
            <Separator
              orientation="vertical"
              className="mr-2 data-vertical:h-4 data-vertical:self-auto"
            />
            <Breadcrumb>
              <BreadcrumbList>
                {pathname !== "/dashboard" ? (
                  <>
                    <BreadcrumbItem className="hidden md:block">
                      <BreadcrumbLink asChild>
                        <Link href="/dashboard">Dashboard</Link>
                      </BreadcrumbLink>
                    </BreadcrumbItem>
                    <BreadcrumbSeparator className="hidden md:block" />
                  </>
                ) : null}
                <BreadcrumbItem>
                  <BreadcrumbPage>{meta.title}</BreadcrumbPage>
                </BreadcrumbItem>
              </BreadcrumbList>
            </Breadcrumb>
          </div>
          {meta.showPageHeader !== false ? (
            <div className="border-t px-4 py-4">
              <div className="flex flex-col gap-3 md:flex-row md:items-end md:justify-between">
                <div className="space-y-2">
                  <p className="text-[0.7rem] font-medium uppercase tracking-[0.24em] text-muted-foreground">
                    {meta.section}
                  </p>
                  <div className="space-y-1">
                    <h1 className="text-2xl font-semibold tracking-tight">
                      {meta.title}
                    </h1>
                    <p className="max-w-3xl text-sm text-muted-foreground">
                      {meta.description}
                    </p>
                  </div>
                </div>
                <div className="rounded-lg border bg-muted/40 px-3 py-2 text-xs text-muted-foreground">
                  Shared page header
                </div>
              </div>
            </div>
          ) : null}
        </header>
        <div className="flex flex-1 flex-col gap-4 p-4 pt-0">{children}</div>
      </SidebarInset>
    </SidebarProvider>
  );
}
