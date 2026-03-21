"use client";

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { AnalyticsRange } from "@/lib/types/analytics";

const RANGE_OPTIONS: Array<{ label: string; value: AnalyticsRange }> = [
  { label: "7D", value: "LAST_7_DAYS" },
  { label: "30D", value: "LAST_30_DAYS" },
  { label: "YTD", value: "YEAR_TO_DATE" },
  { label: "1Y", value: "LAST_1_YEAR" },
];

export function DashboardRangeSelect({
  value,
  onValueChange,
}: {
  value: AnalyticsRange;
  onValueChange: (value: AnalyticsRange) => void;
}) {
  return (
    <Select value={value} onValueChange={(v) => onValueChange(v as AnalyticsRange)}>
      <SelectTrigger className="h-8 w-20 rounded-lg text-xs">
        <SelectValue placeholder="Range" />
      </SelectTrigger>
      <SelectContent>
        {RANGE_OPTIONS.map((option) => (
          <SelectItem key={option.value} value={option.value}>
            {option.label}
          </SelectItem>
        ))}
      </SelectContent>
    </Select>
  );
}
