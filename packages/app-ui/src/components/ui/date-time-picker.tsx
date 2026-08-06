"use client";

import { Calendar01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { format } from "date-fns";
import * as React from "react";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { Calendar } from "@tradstry/app-ui/components/ui/calendar";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@tradstry/app-ui/components/ui/popover";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@tradstry/app-ui/components/ui/select";
import { cn } from "@tradstry/app-ui/lib/utils";

const HOURS = Array.from({ length: 24 }, (_, i) => String(i).padStart(2, "0"));
const MINUTES = Array.from({ length: 60 }, (_, i) =>
  String(i).padStart(2, "0"),
);

const pad = (n: number) => String(n).padStart(2, "0");

/** `YYYY-MM-DDTHH:mm` -> local Date. Parsed field-by-field so the browser never
 *  reinterprets the string as UTC and shifts the day. */
function parseValue(value: string): Date | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(value);
  if (!match) return null;
  const [, y, mo, d, h, mi] = match;
  const date = new Date(
    Number(y),
    Number(mo) - 1,
    Number(d),
    Number(h),
    Number(mi),
  );
  return Number.isNaN(date.getTime()) ? null : date;
}

function toValue(date: Date): string {
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(
    date.getDate(),
  )}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

export interface DateTimePickerProps {
  /** `YYYY-MM-DDTHH:mm`, or `""` when unset. Same contract as `datetime-local`. */
  value: string;
  onChange: (value: string) => void;
  id?: string;
  placeholder?: string;
  disabled?: boolean;
  className?: string;
}

export function DateTimePicker({
  value,
  onChange,
  id,
  placeholder = "Pick a date and time",
  disabled = false,
  className,
}: DateTimePickerProps) {
  const [open, setOpen] = React.useState(false);
  const selected = parseValue(value);

  const hour = selected ? pad(selected.getHours()) : "09";
  const minute = selected ? pad(selected.getMinutes()) : "30";

  function commit(date: Date, h: string, m: string) {
    const next = new Date(date);
    next.setHours(Number(h), Number(m), 0, 0);
    onChange(toValue(next));
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          id={id}
          type="button"
          variant="outline"
          disabled={disabled}
          className={cn(
            "h-9 w-full justify-start gap-2 border-input bg-input/20 px-3 font-normal",
            !selected && "text-muted-foreground",
            className,
          )}
        >
          <HugeiconsIcon icon={Calendar01Icon} className="size-4 shrink-0" />
          {selected ? format(selected, "MMM d, yyyy  HH:mm") : placeholder}
        </Button>
      </PopoverTrigger>

      <PopoverContent className="w-auto p-0" align="start">
        <Calendar
          mode="single"
          selected={selected ?? undefined}
          defaultMonth={selected ?? undefined}
          onSelect={(date) => {
            if (date) commit(date, hour, minute);
          }}
          autoFocus
        />

        <div className="flex items-center gap-2 border-t border-border p-3">
          <span className="text-xs text-muted-foreground">Time</span>
          <Select
            value={hour}
            onValueChange={(h) => commit(selected ?? new Date(), h, minute)}
          >
            <SelectTrigger className="h-8 w-[4.5rem]" aria-label="Hour">
              <SelectValue />
            </SelectTrigger>
            <SelectContent className="max-h-56">
              {HOURS.map((h) => (
                <SelectItem key={h} value={h}>
                  {h}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
          <span className="text-muted-foreground">:</span>
          <Select
            value={minute}
            onValueChange={(m) => commit(selected ?? new Date(), hour, m)}
          >
            <SelectTrigger className="h-8 w-[4.5rem]" aria-label="Minute">
              <SelectValue />
            </SelectTrigger>
            <SelectContent className="max-h-56">
              {MINUTES.map((m) => (
                <SelectItem key={m} value={m}>
                  {m}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
      </PopoverContent>
    </Popover>
  );
}
