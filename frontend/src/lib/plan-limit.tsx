"use client";

import { toast } from "sonner";
import { asPlanLimitError } from "@/lib/types/billing";

function formatResetDate(iso: string | null): string {
  if (!iso) return "";
  const date = new Date(iso);
  return Number.isNaN(date.getTime())
    ? ""
    : date.toLocaleDateString(undefined, { day: "numeric", month: "short" });
}

/**
 * Show a plan-limit error as an upgrade prompt rather than a generic failure.
 *
 * Returns true when the error *was* a plan limit, so callers can skip their own
 * error handling: `if (showPlanLimit(e)) return;`
 */
export function showPlanLimit(error: unknown, onUpgrade?: () => void): boolean {
  const limit = asPlanLimitError(error);
  if (!limit) return false;

  const resets = formatResetDate(limit.resetsAt);

  toast.error(`You've reached your ${limit.resource} limit`, {
    description: resets
      ? `Your plan includes ${limit.limit}. Resets ${resets}.`
      : `Your plan includes ${limit.limit}.`,
    action: onUpgrade ? { label: "Upgrade", onClick: onUpgrade } : undefined,
    duration: 8000,
  });

  return true;
}
