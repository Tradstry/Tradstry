/**
 * Playbook rules are stored as a single TEXT column, and four things read them: the web
 * form, the desktop form, the MCP `get_playbook` tool, and the desktop's last-writer-wins
 * sync. So the structure lives *inside* the text rather than in the schema — a JSON column
 * would break the sync merge and the MCP payload, and every existing playbook with it.
 *
 * A line that starts with a bullet or a number is a list item; everything else is free-form
 * prose. That means the plaintext stays readable everywhere, an old playbook parses without
 * a migration, and a playbook written by hand (or by an agent over MCP) still round-trips.
 */

export type RuleSection = {
  items: string[];
  notes: string;
};

/** `- x`, `* x`, `• x`, `1. x`, `2) x` — the shapes people actually type. */
const ITEM_LINE = /^\s*(?:[-*•]|\d+[.)])\s+(.*)$/;

export function parseRules(text: string): RuleSection {
  const items: string[] = [];
  const noteLines: string[] = [];

  for (const line of (text ?? "").split("\n")) {
    const match = line.match(ITEM_LINE);
    if (match?.[1].trim()) {
      items.push(match[1].trim());
    } else {
      noteLines.push(line);
    }
  }

  return { items, notes: noteLines.join("\n").trim() };
}

export function serializeRules({ items, notes }: RuleSection): string {
  const kept = items.map((i) => i.trim()).filter(Boolean);
  const numbered = kept.map((item, i) => `${i + 1}. ${item}`).join("\n");
  const trimmedNotes = notes.trim();

  if (!numbered) return trimmedNotes;
  if (!trimmedNotes) return numbered;
  return `${numbered}\n\n${trimmedNotes}`;
}

/** Whether the section holds anything at all — the forms require some rules. */
export function isRulesEmpty(section: RuleSection): boolean {
  return serializeRules(section).trim().length === 0;
}
