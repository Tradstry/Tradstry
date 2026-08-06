"use client";

import { ScrollArea } from "@/components/ui/scroll-area";
import { parseRules } from "@/lib/playbook-rules";

/**
 * The read-only half of the rules editor.
 *
 * The card used to print each field into a single `<p>`, which collapses newlines — so a
 * list of rules arrived as one run-on sentence ("1. hh 2. hh First close of the 10-day
 * EMA…"). Same source of truth as the editor: numbered lines are items, the rest is prose.
 */
function RuleGroup({ label, value }: { label: string; value: string }) {
  const { items, notes } = parseRules(value ?? "");
  if (items.length === 0 && !notes) return null;

  return (
    <section className="grid gap-1.5">
      <h4 className="text-[0.63rem] font-semibold uppercase tracking-[0.14em] text-muted-foreground">
        {label}
      </h4>

      {items.length > 0 ? (
        <ol className="grid gap-1">
          {items.map((item, index) => (
            <li
              // The text is the content, and two rules can legitimately read the same.
              // biome-ignore lint/suspicious/noArrayIndexKey: order is the identity here
              key={index}
              className="grid grid-cols-[1.1rem_1fr] gap-1.5 text-xs leading-relaxed text-foreground/90"
            >
              <span className="tabular-nums text-muted-foreground">
                {index + 1}.
              </span>
              <span>{item}</span>
            </li>
          ))}
        </ol>
      ) : null}

      {/* pre-line, or the free-form notes collapse into one line all over again. */}
      {notes ? (
        <p className="whitespace-pre-line text-xs leading-relaxed text-muted-foreground">
          {notes}
        </p>
      ) : null}
    </section>
  );
}

/**
 * A playbook's four rule fields, in a fixed-height scroll region.
 *
 * A real playbook runs to a dozen rules; left to grow, one card's rules push the whole grid
 * out of alignment and bury the performance numbers. The height is deliberately a fixed
 * `h-52` rather than a `max-h`: Radix's scroll viewport is `height: 100%`, which resolves to
 * `auto` against a max-height parent — the content would simply grow and never scroll.
 */
export function RulesView({
  entryRules,
  exitRules,
  positionSizingRules,
  additionalRules,
}: {
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules?: string | null;
}) {
  return (
    <ScrollArea className="h-52 -mr-2 pr-2">
      <div className="grid gap-3 pb-1">
        <RuleGroup label="Entry" value={entryRules} />
        <RuleGroup label="Exit" value={exitRules} />
        <RuleGroup label="Position sizing" value={positionSizingRules} />
        <RuleGroup label="Additional" value={additionalRules ?? ""} />
      </div>
    </ScrollArea>
  );
}
