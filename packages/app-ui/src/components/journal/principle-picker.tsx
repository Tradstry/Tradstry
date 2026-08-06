"use client";

import { UnfoldMoreIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { Checkbox } from "@tradstry/app-ui/components/ui/checkbox";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@tradstry/app-ui/components/ui/popover";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import { usePrinciples } from "@tradstry/app-ui/hooks/principle";
import { cn } from "@tradstry/app-ui/lib/utils";

export interface PrinciplePickerProps {
  /** The trade's account. Principles are account-scoped. */
  workspaceId: string | null;
  /** The trade's currently selected playbook, or null. */
  selectedPlaybookId: string | null;
  /** Currently ticked principle ids. */
  value: string[];
  onChange: (ids: string[]) => void;
  className?: string;
}

/**
 * Multi-select of the principles a trade violated, styled to match the sibling
 * Playbook `Select`. A real `Select` cannot be used: it holds a single value.
 *
 * Only active principles governing this trade are offered: account-wide ones
 * (`playbookId === null`) plus those scoped to the selected playbook.
 */
export function PrinciplePicker({
  workspaceId,
  selectedPlaybookId,
  value,
  onChange,
  className,
}: PrinciplePickerProps) {
  const [open, setOpen] = React.useState(false);
  const principlesQuery = usePrinciples(workspaceId);
  const all = principlesQuery.data ?? [];

  const applicable = React.useMemo(
    () =>
      all
        .filter(
          (p) =>
            p.isActive &&
            (p.playbookId === null || p.playbookId === selectedPlaybookId),
        )
        // Workspace-wide first, then playbook-scoped; each already priority DESC.
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

  const selected = applicable.filter((p) => value.includes(p.id));
  const label =
    selected.length === 0
      ? "None broken"
      : selected.length === 1
        ? (selected[0]?.title ?? "")
        : `${selected.length} principles broken`;

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <button
          type="button"
          aria-label="Principles broken"
          className={cn(
            // Mirrors SelectTrigger exactly, including its `w-fit` sizing.
            "flex h-7 w-fit items-center justify-between gap-1.5 rounded-md border border-input bg-input/20 px-2 py-1.5 text-xs/relaxed whitespace-nowrap outline-none transition-colors focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30 dark:bg-input/30 dark:hover:bg-input/50",
            selected.length === 0 && "text-muted-foreground",
            className,
          )}
        >
          <span className="line-clamp-1 text-left">{label}</span>
          <HugeiconsIcon
            icon={UnfoldMoreIcon}
            strokeWidth={2}
            className="pointer-events-none size-3.5 shrink-0 text-muted-foreground"
          />
        </button>
      </PopoverTrigger>

      <PopoverContent
        className="w-auto min-w-(--radix-popover-trigger-width) p-1"
        align="start"
      >
        <ScrollArea
          className="[&>[data-radix-scroll-area-viewport]]:max-h-56"
          role="listbox"
          aria-multiselectable="true"
          aria-label="Principles broken"
        >
          {applicable.map((principle) => {
            const checked = value.includes(principle.id);
            return (
              <button
                key={principle.id}
                type="button"
                role="option"
                aria-selected={checked}
                onClick={() => toggle(principle.id)}
                className={cn(
                  "flex w-full cursor-pointer items-center gap-2 rounded-sm px-2 py-1.5 text-left text-xs transition-colors hover:bg-muted",
                  checked && "bg-muted/60",
                )}
              >
                <Checkbox
                  checked={checked}
                  tabIndex={-1}
                  aria-hidden="true"
                  className="pointer-events-none"
                />
                <span className="line-clamp-1 flex-1">{principle.title}</span>
              </button>
            );
          })}
        </ScrollArea>
      </PopoverContent>
    </Popover>
  );
}
