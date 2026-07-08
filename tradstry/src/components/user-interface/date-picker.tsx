import { useState } from "react";
import { CalendarIcon } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Calendar } from "@/components/ui/calendar";
import { Label } from "@/components/ui/label";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";

function pad(n: number) {
  return String(n).padStart(2, "0");
}

/** Parse a `YYYY-MM-DDTHH:mm` string into a Date (local), or undefined. */
function parseDate(value: string): Date | undefined {
  if (!value) return undefined;
  const d = new Date(value);
  return Number.isNaN(d.getTime()) ? undefined : d;
}

/** Combine a calendar day with an `HH:mm` time into `YYYY-MM-DDTHH:mm`. */
function toValue(date: Date, time: string): string {
  const [h = "00", m = "00"] = time.split(":");
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${h}:${m}`;
}

function fmtTrigger(value: string): string {
  const d = parseDate(value);
  if (!d) return "";
  return d.toLocaleString("en-US", {
    month: "short",
    day: "numeric",
    year: "numeric",
    hour: "numeric",
    minute: "2-digit",
  });
}

type DateTimePickerProps = {
  label: string;
  /** `YYYY-MM-DDTHH:mm` string, or empty. */
  value: string;
  onChange: (value: string) => void;
};

/**
 * Date + time picker built on the shadcn Calendar in a liquid-glass popover —
 * replaces the browser's native datetime-local control. Emits the same
 * `YYYY-MM-DDTHH:mm` string the form/back end expects.
 */
export function DateTimePicker({ label, value, onChange }: DateTimePickerProps) {
  const [open, setOpen] = useState(false);
  const selected = parseDate(value);
  const time = value ? value.slice(11, 16) || "00:00" : "00:00";

  return (
    <div className="flex flex-col gap-2">
      <Label>{label}</Label>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            className="h-9 w-full justify-between bg-muted/50 px-2.5 font-normal"
          >
            <span className={value ? "" : "text-muted-foreground"}>
              {value ? fmtTrigger(value) : "Pick date & time"}
            </span>
            <CalendarIcon className="size-3.5 text-muted-foreground" />
          </Button>
        </PopoverTrigger>
        <PopoverContent align="start" className="w-auto gap-0 p-0">
          <Calendar
            mode="single"
            selected={selected}
            defaultMonth={selected}
            onSelect={(d) => {
              if (d) onChange(toValue(d, time));
            }}
          />
          <div className="flex items-center gap-2 border-t border-border/60 p-3">
            <Label className="text-xs text-muted-foreground">Time</Label>
            <input
              type="time"
              value={time}
              onChange={(e) =>
                onChange(toValue(selected ?? new Date(), e.target.value))
              }
              className="ml-auto h-8 rounded-lg border border-input bg-transparent px-2.5 text-sm tabular-nums outline-none focus-visible:border-ring focus-visible:ring-3 focus-visible:ring-ring/50 dark:bg-input/30 [&::-webkit-calendar-picker-indicator]:opacity-60"
            />
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
