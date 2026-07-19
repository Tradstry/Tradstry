"use client";

import { CreditCardIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Section, Spinner } from "@/components/account/shared";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  useBilling,
  useBillingPortal,
  useRefreshBilling,
} from "@/hooks/billing";
import { useGraphQL } from "@/lib/client";
import { openCheckout } from "@/lib/paddle";
import type { BillingInfo, Meter, PlanId } from "@/lib/types/billing";
import { cn } from "@/lib/utils";

const PLAN_LABEL: Record<PlanId, string> = {
  free: "Free",
  pro: "Pro",
  pro_plus: "Pro Plus",
};

/** Bars turn amber here so a limit is visible before it bites. */
const WARN_AT = 0.8;

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const mb = bytes / (1024 * 1024);
  if (mb < 1024) return `${mb.toFixed(mb < 10 ? 1 : 0)} MB`;
  return `${(mb / 1024).toFixed(1)} GB`;
}

function formatDate(iso: string): string {
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? ""
    : date.toLocaleDateString(undefined, {
        day: "numeric",
        month: "short",
        year: "numeric",
      });
}

function UsageBar({
  label,
  meter,
  format,
}: {
  label: string;
  meter: Meter;
  format: (value: number) => string;
}) {
  const limit = meter.limit;
  const unlimited = limit === null;
  const ratio = unlimited ? 0 : Math.min(1, meter.used / Math.max(1, limit));
  const exhausted = !unlimited && meter.used >= limit;
  const warning = !unlimited && ratio >= WARN_AT;

  return (
    <div className="space-y-1.5">
      <div className="flex items-baseline justify-between gap-2">
        <span className="text-xs text-muted-foreground">{label}</span>
        <span
          className={cn(
            "text-xs tabular-nums",
            exhausted
              ? "text-destructive"
              : warning
                ? "text-amber-600"
                : "text-foreground",
          )}
        >
          {format(meter.used)}
          {unlimited ? "" : ` / ${format(meter.limit ?? 0)}`}
        </span>
      </div>
      <div className="h-1.5 overflow-hidden rounded-full bg-muted">
        <div
          className={cn(
            "h-full rounded-full transition-[width]",
            exhausted
              ? "bg-destructive"
              : warning
                ? "bg-amber-500"
                : "bg-primary",
          )}
          style={{ width: `${Math.max(unlimited ? 0 : 2, ratio * 100)}%` }}
        />
      </div>
    </div>
  );
}

function StatusNote({ billing }: { billing: BillingInfo }) {
  if (billing.cancelsAtPeriodEnd) {
    return (
      <p className="text-xs text-muted-foreground">
        Your plan ends on {formatDate(billing.periodEnd)}. You keep{" "}
        {PLAN_LABEL[billing.plan]} until then.
      </p>
    );
  }
  if (billing.status === "past_due") {
    return (
      <p className="text-xs text-destructive">
        We couldn&apos;t take your last payment. Update your card to keep your
        plan.
      </p>
    );
  }
  return (
    <p className="text-xs text-muted-foreground">
      Usage resets on {formatDate(billing.periodEnd)}.
    </p>
  );
}

export function PlanSection() {
  const { data: billing, isLoading } = useBilling();
  const portal = useBillingPortal();
  const refreshBilling = useRefreshBilling();
  const fetcher = useGraphQL();
  const [upgrading, setUpgrading] = React.useState<PlanId | null>(null);

  async function upgrade(plan: PlanId) {
    setUpgrading(plan);
    try {
      await openCheckout(fetcher, plan, refreshBilling);
    } catch (error) {
      toast.error(
        error instanceof Error ? error.message : "Could not open checkout.",
      );
    } finally {
      setUpgrading(null);
    }
  }

  if (isLoading || !billing) {
    return (
      <div className="flex items-center justify-center py-10 text-muted-foreground">
        <Spinner />
      </div>
    );
  }

  const isFree = billing.plan === "free";

  return (
    <div className="space-y-4">
      <Section
        title="Current plan"
        description="Every plan has the same features. Paid tiers raise the limits."
        footer={
          <div className="flex flex-wrap items-center gap-2">
            {isFree ? (
              <>
                <Button
                  size="sm"
                  disabled={upgrading !== null}
                  onClick={() => upgrade("pro")}
                >
                  {upgrading === "pro" ? <Spinner /> : null}
                  Upgrade to Pro
                </Button>
                <Button
                  size="sm"
                  variant="outline"
                  disabled={upgrading !== null}
                  onClick={() => upgrade("pro_plus")}
                >
                  {upgrading === "pro_plus" ? <Spinner /> : null}
                  Pro Plus
                </Button>
              </>
            ) : null}
            {billing.plan === "pro" ? (
              <Button
                size="sm"
                disabled={upgrading !== null}
                onClick={() => upgrade("pro_plus")}
              >
                {upgrading === "pro_plus" ? <Spinner /> : null}
                Upgrade to Pro Plus
              </Button>
            ) : null}
            {!isFree ? (
              <Button
                size="sm"
                variant="outline"
                disabled={portal.isPending}
                onClick={() => portal.mutate()}
              >
                {portal.isPending ? (
                  <Spinner />
                ) : (
                  <HugeiconsIcon
                    icon={CreditCardIcon}
                    strokeWidth={2}
                    className="size-4"
                  />
                )}
                Manage billing
              </Button>
            ) : null}
          </div>
        }
      >
        <div className="flex items-center gap-2">
          <span className="text-lg font-medium">
            {PLAN_LABEL[billing.plan]}
          </span>
          {billing.cancelsAtPeriodEnd ? (
            <Badge variant="outline">Ending soon</Badge>
          ) : billing.status === "past_due" ? (
            <Badge variant="destructive">Payment failed</Badge>
          ) : null}
        </div>
        <div className="mt-2">
          <StatusNote billing={billing} />
        </div>
      </Section>

      <Section
        title="Usage"
        description="What you've used in the current period."
      >
        <div className="space-y-4">
          <UsageBar
            label="AI actions"
            meter={billing.meters.ai}
            format={(v) => String(v)}
          />
          <UsageBar
            label="Brokerage connections"
            meter={billing.meters.connections}
            format={(v) => String(v)}
          />
          <UsageBar
            label="Trade data"
            meter={billing.meters.data}
            format={formatBytes}
          />
          <UsageBar
            label="Images & video"
            meter={billing.meters.media}
            format={formatBytes}
          />
        </div>
      </Section>
    </div>
  );
}
