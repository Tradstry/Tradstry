import type * as React from "react";

export function DashboardRoutePage({ title }: { title: string }) {
  return (
    <>
      <section className="grid gap-4 md:grid-cols-3">
        <div className="rounded-xl border bg-muted/40 p-4">
          <p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
            Overview
          </p>
          <p className="mt-2 text-sm text-muted-foreground">
            This route is now wired into the sidebar and ready for real feature
            work.
          </p>
        </div>
        <div className="rounded-xl border bg-muted/40 p-4">
          <p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
            Next Step
          </p>
          <p className="mt-2 text-sm text-muted-foreground">
            Add the actual data modules and interactions for{" "}
            {title.toLowerCase()}.
          </p>
        </div>
        <div className="rounded-xl border bg-muted/40 p-4">
          <p className="text-xs font-medium uppercase tracking-[0.16em] text-muted-foreground">
            Status
          </p>
          <p className="mt-2 text-sm text-muted-foreground">
            Navigation, layout, and breadcrumbs are connected.
          </p>
        </div>
      </section>
      <section className="min-h-[24rem] rounded-xl border bg-background p-6">
        <div className="max-w-2xl space-y-3">
          <h2 className="text-lg font-medium">{title} workspace</h2>
          <p className="text-sm text-muted-foreground">
            This is the route shell for {title}. Replace this placeholder with
            the actual UI for the feature when you are ready.
          </p>
        </div>
      </section>
    </>
  );
}
