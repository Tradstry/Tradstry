"use client";

import type { useUser } from "@clerk/nextjs";
import { Loading03Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type * as React from "react";
import { Label } from "@tradstry/app-ui/components/ui/label";
import { cn } from "@tradstry/app-ui/lib/utils";

export type ClerkUser = NonNullable<ReturnType<typeof useUser>["user"]>;
export type EmailResource = ClerkUser["emailAddresses"][number];
export type ExternalAccount = ClerkUser["externalAccounts"][number];
export type DeviceSession = Awaited<
  ReturnType<ClerkUser["getSessions"]>
>[number];

export function clerkError(err: unknown, fallback: string): string {
  const errors = (
    err as { errors?: Array<{ longMessage?: string; message?: string }> } | null
  )?.errors;
  return errors?.[0]?.longMessage ?? errors?.[0]?.message ?? fallback;
}

export function Section({
  title,
  description,
  children,
  footer,
  tone = "default",
}: {
  title: string;
  description?: string;
  children: React.ReactNode;
  footer?: React.ReactNode;
  tone?: "default" | "destructive";
}) {
  return (
    <section
      className={cn(
        "overflow-hidden rounded-xl border bg-card",
        tone === "destructive" ? "border-destructive/40" : "border-border/60",
      )}
    >
      <header className="px-4 pt-4">
        <h3
          className={cn(
            "text-sm font-medium",
            tone === "destructive" && "text-destructive",
          )}
        >
          {title}
        </h3>
        {description ? (
          <p className="mt-1 text-xs leading-relaxed text-muted-foreground">
            {description}
          </p>
        ) : null}
      </header>
      <div className="px-4 py-4">{children}</div>
      {footer ? (
        <div className="flex items-center justify-end gap-2 border-t border-border/60 bg-muted/30 px-4 py-3">
          {footer}
        </div>
      ) : null}
    </section>
  );
}

export function Field({
  label,
  htmlFor,
  error,
  hint,
  children,
}: {
  label: string;
  htmlFor: string;
  error?: string | null;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid gap-1.5">
      <Label htmlFor={htmlFor} className="text-xs text-muted-foreground">
        {label}
      </Label>
      {children}
      {error ? (
        <p role="alert" className="text-xs text-destructive">
          {error}
        </p>
      ) : hint ? (
        <p className="text-xs text-muted-foreground">{hint}</p>
      ) : null}
    </div>
  );
}

export function Spinner({ className }: { className?: string }) {
  return (
    <HugeiconsIcon
      icon={Loading03Icon}
      strokeWidth={2}
      className={cn("size-4 animate-spin", className)}
    />
  );
}

export function EmptyRow({ children }: { children: React.ReactNode }) {
  return (
    <p className="rounded-lg border border-dashed border-border/60 px-3 py-6 text-center text-xs text-muted-foreground">
      {children}
    </p>
  );
}
