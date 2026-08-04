"use client";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { capture, EVENTS } from "@/lib/analytics/events";
import { RANGE_PRESETS } from "@/lib/range-presets";
import type { AnalyticsRange } from "@/lib/types/analytics";

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
