"use client";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { AnalyticsRange } from "@/lib/types/analytics";
import { RANGE_PRESETS } from "@/lib/range-presets";

export function DashboardRangeSelect({
  value,
  onValueChange,
}: {
  value: AnalyticsRange;
  onValueChange: (value: AnalyticsRange) => void;
}) {
  return (
    <Select value={value} onValueChange={(v) => onValueChange(v as AnalyticsRange)}>
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
