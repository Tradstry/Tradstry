import { Label } from "@/components/ui/label";

/** Allows only a (partial) decimal number — including mid-typing states like
 *  "", "-", "1." — so the field never accepts letters and needs no spinners. */
const NUMERIC = /^-?\d*\.?\d*$/;

type NumberFieldProps = {
  label: string;
  /** String-backed value (empty string = no value). */
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  /** Leading adornment, e.g. "$" for a price. */
  prefix?: string;
  /** Trailing adornment, e.g. "shares" for a quantity. */
  suffix?: string;
};

/**
 * Numeric input styled with shadcn tokens — no native spinner arrows, accepts
 * only numbers, with optional unit adornments (matches ui/input's look).
 */
export function NumberField({
  label,
  value,
  onChange,
  placeholder,
  prefix,
  suffix,
}: NumberFieldProps) {
  return (
    <div className="flex flex-col gap-2">
      <Label>{label}</Label>
      <div className="flex h-9 items-center rounded-lg border border-input bg-muted/50 transition-colors focus-within:border-ring focus-within:ring-3 focus-within:ring-ring/50">
        {prefix && (
          <span className="pl-2.5 text-sm text-muted-foreground">{prefix}</span>
        )}
        <input
          inputMode="decimal"
          placeholder={placeholder}
          value={value}
          onChange={(e) => {
            const v = e.target.value;
            if (NUMERIC.test(v)) onChange(v);
          }}
          className={`h-full w-full min-w-0 bg-transparent ${prefix ? "pl-1" : "pl-2.5"} ${suffix ? "pr-1" : "pr-2.5"} text-sm tabular-nums outline-none placeholder:text-muted-foreground`}
        />
        {suffix && (
          <span className="whitespace-nowrap pr-2.5 text-xs text-muted-foreground">
            {suffix}
          </span>
        )}
      </div>
    </div>
  );
}
