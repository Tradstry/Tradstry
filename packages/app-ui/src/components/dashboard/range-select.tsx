"use client";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { capture, EVENTS } from "@tradstry/app-ui/lib/analytics/events";
import { RANGE_PRESETS } from "@tradstry/app-ui/lib/range-presets";
import type { AnalyticsRange } from "@tradstry/app-ui/lib/types/analytics";

export function DashboardRangeSelect({
  value,
  onValueChange,
}: {
  value: AnalyticsRange;
  onValueChange: (value: AnalyticsRange) => void;
}) {
  return (
    <Select
      value={value}
      onValueChange={(v) => {
        capture(EVENTS.analyticsRangeChanged, { range: v });
        onValueChange(v as AnalyticsRange);
      }}
    >
      <SelectTrigger className="h-8 w-24 rounded-lg text-xs">
        <SelectValue placeholder="Range" />
      </SelectTrigger>
      <SelectContent>
        {RANGE_PRESETS.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
