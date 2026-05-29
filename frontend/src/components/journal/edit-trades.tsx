"use client";

import { PencilEdit01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { TagPicker } from "@/components/journal/tag-picker";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useUpdateJournalEntry } from "@/hooks/journal";
import { usePlaybooks } from "@/hooks/playbook";
import { useTagCategories } from "@/hooks/tags";
import type { JournalEntry, TradeType } from "@/lib/types/journal";
import { cn } from "@/lib/utils";

type TradeFormState = {
  symbol: string;
  symbolName: string;
  openDate: string;
  closeDate: string;
  entryPrice: string;
  exitPrice: string;
  positionSize: string;
  stopLoss: string;
  tradeType: TradeType;
  playbookId: string;
  notes: string;
};

const NO_PLAYBOOK_VALUE = "__none__";

function toLocalDateTime(value: string) {
  const parsed = new Date(value);
  if (Number.isNaN(parsed.getTime())) {
    return value;
  }

  const offset = parsed.getTimezoneOffset();
  const local = new Date(parsed.getTime() - offset * 60_000);
  return local.toISOString().slice(0, 16);
}

function createInitialState(trade: JournalEntry): TradeFormState {
  return {
    symbol: trade.symbol,
    symbolName: trade.symbolName,
    openDate: toLocalDateTime(trade.openDate),
    closeDate: toLocalDateTime(trade.closeDate),
    entryPrice: String(trade.entryPrice),
    exitPrice: String(trade.exitPrice),
    positionSize: String(trade.positionSize),
    stopLoss: String(trade.stopLoss),
    tradeType: trade.tradeType,
    playbookId: trade.playbookId ?? "",
    notes: trade.notes ?? "",
  };
}

/** Build the initial tagIdsByCategory map from a trade's tags array. */
function buildInitialTagIdsByCategory(
  trade: JournalEntry,
): Record<string, string[]> {
  const map: Record<string, string[]> = {};
  for (const tag of trade.tags) {
    const list = map[tag.categoryId];
    if (list) {
      list.push(tag.id);
    } else {
      map[tag.categoryId] = [tag.id];
    }
  }
  return map;
}

function Field({
  label,
  htmlFor,
  children,
}: {
  label: string;
  htmlFor?: string;
  children: React.ReactNode;
}) {
  return (
    <div className="grid gap-2">
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  );
}

interface EditTradesProps {
  trade: JournalEntry;
}

export function EditTrades({ trade }: EditTradesProps) {
  const updateTrade = useUpdateJournalEntry();
  const playbooks = usePlaybooks();
  const tagCategories = useTagCategories();
  const availablePlaybooks = playbooks.data ?? [];
  const [open, setOpen] = React.useState(false);
  const [form, setForm] = React.useState<TradeFormState>(() =>
    createInitialState(trade),
  );
  const [tagIdsByCategory, setTagIdsByCategory] = React.useState<
    Record<string, string[]>
  >(() => buildInitialTagIdsByCategory(trade));
  const [error, setError] = React.useState("");
  const selectedPlaybook = availablePlaybooks.find(
    (playbook) => playbook.id === form.playbookId,
  );

  React.useEffect(() => {
    if (open) {
      setForm(createInitialState(trade));
      setTagIdsByCategory(buildInitialTagIdsByCategory(trade));
      setError("");
    }
  }, [open, trade]);

  function setField<K extends keyof TradeFormState>(
    key: K,
    value: TradeFormState[K],
  ) {
    setForm((current) => ({ ...current, [key]: value }));
    if (error) {
      setError("");
    }
  }

  function validateForm() {
    if (!form.symbol.trim()) return "Symbol is required";
    if (!form.openDate) return "Open date is required";
    if (!form.closeDate) return "Close date is required";
    if (!form.entryPrice.trim()) return "Entry price is required";
    if (!form.exitPrice.trim()) return "Exit price is required";
    if (!form.positionSize.trim()) return "Position size is required";
    if (!form.stopLoss.trim()) return "Stop loss is required";
    return "";
  }

  async function handleSubmit(event: React.FormEvent<HTMLFormElement>) {
    event.preventDefault();

    const validationError = validateForm();
    if (validationError) {
      setError(validationError);
      return;
    }

    const trimmedNotes = form.notes.trim();
    const hadPlaybook = Boolean(trade.playbookId);
    const willClearPlaybook = hadPlaybook && !form.playbookId;
    const tagIds = Object.values(tagIdsByCategory).flat();

    try {
      await updateTrade.mutateAsync({
        id: trade.id,
        input: {
          symbol: form.symbol.trim().toUpperCase(),
          symbolName: form.symbolName.trim() || undefined,
          openDate: form.openDate,
          closeDate: form.closeDate,
          entryPrice: Number(form.entryPrice),
          exitPrice: Number(form.exitPrice),
          positionSize: Number(form.positionSize),
          stopLoss: Number(form.stopLoss),
          tradeType: form.tradeType,
          tagIds,
          playbookId: form.playbookId || undefined,
          clearPlaybook: willClearPlaybook,
          notes: trimmedNotes || undefined,
          clearNotes: !trimmedNotes,
        },
      });
      setOpen(false);
    } catch (submissionError) {
      setError(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to update trade",
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          className="text-muted-foreground"
        >
          <HugeiconsIcon
            icon={PencilEdit01Icon}
            strokeWidth={2}
            className="size-4"
          />
          <span className="sr-only">Edit trade</span>
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-3xl">
        <form onSubmit={handleSubmit}>
          <DialogHeader>
            <DialogTitle className="flex items-center gap-2">
              <span className="flex size-7 items-center justify-center rounded-md bg-muted text-muted-foreground">
                <HugeiconsIcon
                  icon={PencilEdit01Icon}
                  strokeWidth={2}
                  className="size-4"
                />
              </span>
              Edit trade
            </DialogTitle>
            <DialogDescription>
              Update this journal entry and save the recalculated metrics.
            </DialogDescription>
          </DialogHeader>

          <div className="grid gap-4 py-4 md:grid-cols-2">
            <Field label="Symbol" htmlFor={`edit-symbol-${trade.id}`}>
              <Input
                id={`edit-symbol-${trade.id}`}
                value={form.symbol}
                onChange={(event) => setField("symbol", event.target.value)}
              />
            </Field>
            <Field label="Symbol Name" htmlFor={`edit-symbol-name-${trade.id}`}>
              <Input
                id={`edit-symbol-name-${trade.id}`}
                value={form.symbolName}
                onChange={(event) => setField("symbolName", event.target.value)}
              />
            </Field>
            <Field label="Open Date" htmlFor={`edit-open-date-${trade.id}`}>
              <Input
                id={`edit-open-date-${trade.id}`}
                type="datetime-local"
                value={form.openDate}
                onChange={(event) => setField("openDate", event.target.value)}
              />
            </Field>
            <Field label="Close Date" htmlFor={`edit-close-date-${trade.id}`}>
              <Input
                id={`edit-close-date-${trade.id}`}
                type="datetime-local"
                value={form.closeDate}
                onChange={(event) => setField("closeDate", event.target.value)}
              />
            </Field>
            <Field label="Entry Price" htmlFor={`edit-entry-price-${trade.id}`}>
              <Input
                id={`edit-entry-price-${trade.id}`}
                type="number"
                step="0.0001"
                min="0"
                value={form.entryPrice}
                onChange={(event) => setField("entryPrice", event.target.value)}
              />
            </Field>
            <Field label="Exit Price" htmlFor={`edit-exit-price-${trade.id}`}>
              <Input
                id={`edit-exit-price-${trade.id}`}
                type="number"
                step="0.0001"
                min="0"
                value={form.exitPrice}
                onChange={(event) => setField("exitPrice", event.target.value)}
              />
            </Field>
            <Field
              label="Position Size"
              htmlFor={`edit-position-size-${trade.id}`}
            >
              <Input
                id={`edit-position-size-${trade.id}`}
                type="number"
                step="0.01"
                min="0"
                value={form.positionSize}
                onChange={(event) =>
                  setField("positionSize", event.target.value)
                }
              />
            </Field>
            <Field label="Stop Loss" htmlFor={`edit-stop-loss-${trade.id}`}>
              <Input
                id={`edit-stop-loss-${trade.id}`}
                type="number"
                step="0.0001"
                min="0"
                value={form.stopLoss}
                onChange={(event) => setField("stopLoss", event.target.value)}
              />
            </Field>
            <Field label="Trade Type">
              <Select
                value={form.tradeType}
                onValueChange={(value) =>
                  setField("tradeType", value as TradeType)
                }
              >
                <SelectTrigger>
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="long">Long</SelectItem>
                  <SelectItem value="short">Short</SelectItem>
                </SelectContent>
              </Select>
            </Field>
            <Field label="Playbook (Optional)">
              <Select
                value={form.playbookId || NO_PLAYBOOK_VALUE}
                onValueChange={(value) =>
                  setField(
                    "playbookId",
                    value === NO_PLAYBOOK_VALUE ? "" : value,
                  )
                }
                disabled={playbooks.isLoading}
              >
                <SelectTrigger>
                  <SelectValue placeholder="No playbook" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value={NO_PLAYBOOK_VALUE}>No playbook</SelectItem>
                  {availablePlaybooks.map((playbook) => (
                    <SelectItem key={playbook.id} value={playbook.id}>
                      {playbook.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
              <p className="text-xs text-muted-foreground">
                {playbooks.isLoading
                  ? "Loading playbooks..."
                  : selectedPlaybook
                    ? `Current playbook: ${selectedPlaybook.name}. You can keep it, change it, or remove it.`
                    : availablePlaybooks.length > 0
                      ? "Select a playbook for this trade or leave it unassigned."
                      : "No playbooks available yet. Create one first if you want to tag this trade."}
              </p>
            </Field>

            <Field label="Notes" htmlFor={`edit-notes-${trade.id}`}>
              <textarea
                id={`edit-notes-${trade.id}`}
                value={form.notes}
                onChange={(event) => setField("notes", event.target.value)}
                rows={4}
                className={cn(
                  "min-h-24 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30",
                )}
              />
            </Field>
          </div>

          {/* Per-category tag pickers */}
          {(tagCategories.data ?? []).length > 0 && (
            <div className="grid gap-4 pb-4 md:grid-cols-2">
              {(tagCategories.data ?? []).map((category) => (
                <Field key={category.id} label={category.name}>
                  <TagPicker
                    category={category}
                    selectedTagIds={tagIdsByCategory[category.id] ?? []}
                    onChange={(ids) =>
                      setTagIdsByCategory((prev) => ({
                        ...prev,
                        [category.id]: ids,
                      }))
                    }
                  />
                </Field>
              ))}
            </div>
          )}

          {error ? (
            <p className="pb-3 text-sm text-destructive">{error}</p>
          ) : null}

          <DialogFooter>
            <Button type="submit" disabled={updateTrade.isPending}>
              {updateTrade.isPending ? "Saving..." : "Save Changes"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
