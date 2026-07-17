"use client";

import { Add01Icon, Delete02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  parseRules,
  type RuleSection,
  serializeRules,
} from "@/lib/playbook-rules";

const notesClass =
  "min-h-16 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30";

/**
 * A rules field: a numbered list you build item by item, plus free-form text for anything
 * that is not a rule — context, caveats, a note to yourself.
 *
 * The value stays a plain string, because that is what the column, the desktop and the MCP
 * tools all read. Structure is parsed out on the way in and written back on the way out.
 */
export function RulesEditor({
  label,
  value,
  onChange,
  placeholder,
  notesPlaceholder = "Anything that isn't a numbered rule…",
}: {
  label: string;
  value: string;
  onChange: (next: string) => void;
  placeholder: string;
  notesPlaceholder?: string;
}) {
  // Parsed from the incoming text once, then owned here: re-parsing on every keystroke
  // would renumber and reflow the list under the user's cursor as they type.
  const [section, setSection] = React.useState<RuleSection>(() =>
    parseRules(value),
  );
  const [draft, setDraft] = React.useState("");
  const addRef = React.useRef<HTMLInputElement>(null);

  const commit = (next: RuleSection) => {
    setSection(next);
    onChange(serializeRules(next));
  };

  const addItem = () => {
    const item = draft.trim();
    if (!item) return;
    commit({ ...section, items: [...section.items, item] });
    setDraft("");
    addRef.current?.focus();
  };

  const editItem = (index: number, text: string) => {
    const items = [...section.items];
    items[index] = text;
    commit({ ...section, items });
  };

  const removeItem = (index: number) => {
    commit({ ...section, items: section.items.filter((_, i) => i !== index) });
  };

  return (
    <div className="grid gap-2">
      <Label>{label}</Label>

      <div className="grid gap-1.5">
        {section.items.map((item, index) => (
          <div
            // Index-keyed on purpose: the text is the value being edited, so keying by it
            // would remount the input on every keystroke and lose focus.
            // biome-ignore lint/suspicious/noArrayIndexKey: see above
            key={index}
            className="flex items-center gap-2"
          >
            <span className="w-5 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
              {index + 1}.
            </span>
            <Input
              value={item}
              onChange={(e) => editItem(index, e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  e.preventDefault();
                  addRef.current?.focus();
                }
                if (e.key === "Backspace" && item === "") {
                  e.preventDefault();
                  removeItem(index);
                }
              }}
              className="h-8 text-sm"
            />
            <Button
              type="button"
              size="icon-sm"
              variant="ghost"
              aria-label={`Remove rule ${index + 1}`}
              className="size-8 shrink-0 text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
              onClick={() => removeItem(index)}
            >
              <HugeiconsIcon icon={Delete02Icon} size={14} strokeWidth={2} />
            </Button>
          </div>
        ))}

        <div className="flex items-center gap-2">
          <span className="w-5 shrink-0 text-right text-xs tabular-nums text-muted-foreground">
            {section.items.length + 1}.
          </span>
          <Input
            ref={addRef}
            value={draft}
            placeholder={placeholder}
            onChange={(e) => setDraft(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                addItem();
              }
            }}
            // A rule typed but never "added" would be silently discarded on save.
            onBlur={addItem}
            className="h-8 text-sm"
          />
          <Button
            type="button"
            size="icon-sm"
            variant="ghost"
            aria-label={`Add rule to ${label}`}
            className="size-8 shrink-0 text-muted-foreground"
            onClick={addItem}
          >
            <HugeiconsIcon icon={Add01Icon} size={14} strokeWidth={2} />
          </Button>
        </div>
      </div>

      <textarea
        value={section.notes}
        placeholder={notesPlaceholder}
        onChange={(e) => commit({ ...section, notes: e.target.value })}
        className={notesClass}
      />
    </div>
  );
}
