import { Label } from "@/components/ui/label";
import {
  Select as ShadSelect,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type Option = { id: string; label: string };

type SelectProps = {
  label?: string;
  value: string;
  onChange: (value: string) => void;
  options: Option[];
  placeholder?: string;
};

/**
 * App-wide dropdown (shadcn Select) — a fully custom, liquid-glass menu that
 * replaces the native OS select. Simple {value, onChange, options} API so it
 * drops into string-backed forms.
 */
export function Select({
  label,
  value,
  onChange,
  options,
  placeholder = "Select…",
}: SelectProps) {
  return (
    <div className="flex flex-col gap-2">
      {label && <Label>{label}</Label>}
      <ShadSelect value={value} onValueChange={onChange}>
        <SelectTrigger className="h-9 w-full bg-muted/50">
          <SelectValue placeholder={placeholder} />
        </SelectTrigger>
        <SelectContent>
          {options.map((o) => (
            <SelectItem key={o.id} value={o.id}>
              {o.label}
            </SelectItem>
          ))}
        </SelectContent>
      </ShadSelect>
    </div>
  );
}
