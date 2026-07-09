"use client";

import * as React from "react";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { usePrinciples } from "@/hooks/principle";
import { cn } from "@/lib/utils";

export interface PrinciplePickerProps {
  /** The trade's account. Principles are account-scoped. */
  accountId: string | null;
  /** The trade's currently selected playbook, or null. */
  selectedPlaybookId: string | null;
  /** Currently ticked principle ids. */
  value: string[];
  onChange: (ids: string[]) => void;
  className?: string;
}

/**
 * Multi-select of the principles a trade violated.
 *
 * Only active principles governing this trade are offered: account-wide ones
 * (`playbookId === null`) plus those scoped to the selected playbook.
 */
export function PrinciplePicker({
  accountId,
  selectedPlaybookId,
  value,
  onChange,
  className,
}: PrinciplePickerProps) {
  const principlesQuery = usePrinciples(accountId);
  const all = principlesQuery.data ?? [];

  const applicable = React.useMemo(
    () =>
      all
        .filter(
          (p) =>
            p.isActive &&
            (p.playbookId === null || p.playbookId === selectedPlaybookId),
        )
        // Account-wide first, then playbook-scoped; each already priority DESC.
        .sort((a, b) => {
          const aGlobal = a.playbookId === null ? 0 : 1;
          const bGlobal = b.playbookId === null ? 0 : 1;
          if (aGlobal !== bGlobal) return aGlobal - bGlobal;
          return b.priority - a.priority;
        }),
    [all, selectedPlaybookId],
  );

  function toggle(id: string) {
    if (value.includes(id)) {
      onChange(value.filter((x) => x !== id));
    } else {
      onChange([...value, id]);
    }
  }

  if (principlesQuery.isLoading) {
    return <p className="text-xs text-muted-foreground">Loading…</p>;
  }

  if (applicable.length === 0) {
    return (
      <p className="text-xs text-muted-foreground">
        No principles yet — add them on the Playbook page.
      </p>
    );
  }

  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      {applicable.map((principle) => (
        <div key={principle.id} className="flex items-center gap-2">
          <Checkbox
            id={`principle-${principle.id}`}
            checked={value.includes(principle.id)}
            onCheckedChange={() => toggle(principle.id)}
          />
          <Label
            htmlFor={`principle-${principle.id}`}
            className="cursor-pointer text-xs font-normal"
          >
            {principle.title}
          </Label>
        </div>
      ))}
    </div>
  );
}
