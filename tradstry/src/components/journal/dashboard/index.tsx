import { useEffect, useState } from "react";
import { Button, Menu, MenuItem, MenuTrigger, Popover } from "react-aria-components";
import { CaretDownIcon } from "@phosphor-icons/react";
import { accounts, type AnalyticsRange } from "../../../backend";
import MetricsGrid from "./metrics";
import Calendar from "./calendar";

const RANGE_OPTIONS: { value: AnalyticsRange; label: string }[] = [
  { value: "TODAY", label: "Today" },
  { value: "LAST_7_DAYS", label: "Last 7 days" },
  { value: "LAST_1_MONTH", label: "Last month" },
  { value: "LAST_3_MONTHS", label: "Last 3 months" },
  { value: "LAST_6_MONTHS", label: "Last 6 months" },
  { value: "YEAR_TO_DATE", label: "Year to date" },
  { value: "LAST_1_YEAR", label: "Last year" },
  { value: "ALL", label: "All time" },
];

function rangeLabel(value: AnalyticsRange) {
  return RANGE_OPTIONS.find((o) => o.value === value)?.label ?? "All time";
}

function RangeSelect({
  value,
  onChange,
}: {
  value: AnalyticsRange;
  onChange: (value: AnalyticsRange) => void;
}) {
  return (
    <MenuTrigger>
      <Button className="flex h-8 cursor-pointer items-center gap-1.5 rounded-lg border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 outline-none transition duration-150 data-hovered:bg-zinc-50 data-focus-visible:outline-2 data-focus-visible:outline-blue-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-200 dark:data-hovered:bg-zinc-800">
        {rangeLabel(value)}
        <CaretDownIcon size={13} className="text-zinc-400 dark:text-zinc-500" />
      </Button>
      <Popover className="w-44 rounded-xl border border-zinc-200 bg-white p-1.5 shadow-lg outline-none dark:border-zinc-800 dark:bg-zinc-900">
        <Menu
          className="outline-none"
          onAction={(key) => onChange(key as AnalyticsRange)}
        >
          {RANGE_OPTIONS.map((o) => (
            <MenuItem
              key={o.value}
              id={o.value}
              className="flex cursor-pointer items-center rounded-md px-2.5 py-1.5 text-sm text-zinc-700 outline-none data-focused:bg-zinc-100 data-focus-visible:bg-zinc-100 dark:text-zinc-200 dark:data-focused:bg-zinc-800 dark:data-focus-visible:bg-zinc-800"
            >
              {o.label}
            </MenuItem>
          ))}
        </Menu>
      </Popover>
    </MenuTrigger>
  );
}

export default function Dashboard() {
  // undefined = resolving, null = no account, string = active account id
  const [accountId, setAccountId] = useState<string | null | undefined>(
    undefined,
  );
  const [range, setRange] = useState<AnalyticsRange>("LAST_1_MONTH");

  useEffect(() => {
    accounts()
      .then((accs) => setAccountId(accs[0]?.id ?? null))
      .catch(() => setAccountId(null));
  }, []);

  if (accountId === undefined) {
    return (
      <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
    );
  }

  if (accountId === null) {
    return (
      <p className="text-sm text-zinc-500 dark:text-zinc-400">
        No account yet — connect a brokerage to see your dashboard.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-6">
      <div className="flex justify-end">
        <RangeSelect value={range} onChange={setRange} />
      </div>
      <MetricsGrid accountId={accountId} range={range} />
      <Calendar accountId={accountId} />
    </div>
  );
}
