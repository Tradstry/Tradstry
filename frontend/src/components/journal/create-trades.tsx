"use client";

import { PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { useActiveAccount } from "@/components/accounts";
import { PrinciplePicker } from "@/components/journal/principle-picker";
import { TagPicker } from "@/components/journal/tag-picker";
import { Button } from "@/components/ui/button";
import { DateTimePicker } from "@/components/ui/date-time-picker";
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
import { ScrollArea } from "@/components/ui/scroll-area";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { useCreateJournalEntry } from "@/hooks/journal";
import { usePlaybooks } from "@/hooks/playbook";
import { usePrinciples } from "@/hooks/principle";
import { useTagCategories } from "@/hooks/tags";
import type { CreateJournalEntryInput, TradeType } from "@/lib/types/journal";
import { cn } from "@/lib/utils";

type Instrument = "stock" | "option";
type OptionKind = "CALL" | "PUT";

type TradeFormState = {
  instrument: Instrument;
  symbol: string;
  symbolName: string;
  optionKind: OptionKind;
  strike: string;
  expiration: string;
  contractMultiplier: string;
  openDate: string;
  closeDate: string;
  entryPrice: string;
  exitPrice: string;
  positionSize: string;
  stopLoss: string;
  tradeType: TradeType;
  playbookId: string;
  notes: string;
  violatedPrincipleIds: string[];
};

const initialFormState: TradeFormState = {
  instrument: "stock",
  symbol: "",
  symbolName: "",
  optionKind: "CALL",
  strike: "",
  expiration: "",
  contractMultiplier: "100",
  openDate: "",
  closeDate: "",
  entryPrice: "",
  exitPrice: "",
  positionSize: "",
  stopLoss: "",
  tradeType: "long",
  playbookId: "",
  notes: "",
  violatedPrincipleIds: [],
};

/** e.g. "AAPL $150 Call 2026-01-17" — a readable contract name from the parts. */
function buildOptionSymbolName(
  underlying: string,
  kind: OptionKind,
  strike: string,
  expiration: string,
): string {
  const parts = [underlying];
  const strikeNum = Number(strike);
  if (Number.isFinite(strikeNum) && strikeNum > 0) parts.push(`$${strike}`);
  parts.push(kind === "PUT" ? "Put" : "Call");
  if (expiration) parts.push(expiration);
  return parts.join(" ").trim();
}

function Field({
  label,
  htmlFor,
  children,
  className,
}: {
  label: string;
  htmlFor?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <div className={cn("grid gap-2", className)}>
      <Label htmlFor={htmlFor}>{label}</Label>
      {children}
    </div>
  );
}

export function CreateTrades({
  open: controlledOpen,
  onOpenChange: controlledOnOpenChange,
  trigger,
}: {
  open?: boolean;
  onOpenChange?: (open: boolean) => void;
  trigger?: React.ReactNode;
} = {}) {
  const activeAccount = useActiveAccount();
  const createTrade = useCreateJournalEntry();
  const playbooks = usePlaybooks();
  const tagCategories = useTagCategories();
  const [internalOpen, setInternalOpen] = React.useState(false);
  const [tagIdsByCategory, setTagIdsByCategory] = React.useState<
    Record<string, string[]>
  >({});

  const open = controlledOpen ?? internalOpen;
  const setOpen = controlledOnOpenChange ?? setInternalOpen;
  const [form, setForm] = React.useState<TradeFormState>(initialFormState);
  const [error, setError] = React.useState("");

  React.useEffect(() => {
    if (!open) {
      setForm(initialFormState);
      setTagIdsByCategory({});
      setError("");
    }
  }, [open]);

  // Principles are account-scoped. A principle from the previous account is
  // not a valid violation for this trade, and the backend rejects the whole
  // create if one slips through.
  const activeAccountId = activeAccount?.id ?? null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: reset only on account change
  React.useEffect(() => {
    setField("violatedPrincipleIds", []);
  }, [activeAccountId]);

  // Changing playbook drops principles scoped to the old playbook; account-wide
  // ones survive.
  const principlesQuery = usePrinciples(activeAccountId);
  const selectedPlaybookId = form.playbookId || null;
  // biome-ignore lint/correctness/useExhaustiveDependencies: prune only on playbook change
  React.useEffect(() => {
    const byId = new Map((principlesQuery.data ?? []).map((p) => [p.id, p]));
    setForm((current) => ({
      ...current,
      violatedPrincipleIds: current.violatedPrincipleIds.filter((id) => {
        const p = byId.get(id);
        return p
          ? p.playbookId === null || p.playbookId === selectedPlaybookId
          : false;
      }),
    }));
  }, [selectedPlaybookId]);

  function setField<K extends keyof TradeFormState>(
    key: K,
    value: TradeFormState[K],
  ) {
    setForm((current) => ({ ...current, [key]: value }));
    if (error) {
      setError("");
    }
  }

  const isOption = form.instrument === "option";

  function validateForm() {
    if (!activeAccount) return "Select an account before creating a trade";
    if (!form.symbol.trim())
      return isOption ? "Underlying is required" : "Symbol is required";
    if (isOption) {
      if (!form.strike.trim()) return "Strike price is required for options";
      if (!form.expiration) return "Expiration date is required for options";
      const m = Number(form.contractMultiplier);
      if (!Number.isFinite(m) || m <= 0)
        return "Contract multiplier must be a positive number";
    }
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
    if (!activeAccount) {
      setError("Select an account before creating a trade");
      return;
    }

    const tagIds = Object.values(tagIdsByCategory).flat();

    const symbol = form.symbol.trim().toUpperCase();
    const composedName = isOption
      ? buildOptionSymbolName(
          symbol,
          form.optionKind,
          form.strike,
          form.expiration,
        )
      : "";

    const input: CreateJournalEntryInput = {
      accountId: activeAccount.id,
      symbol,
      symbolName: form.symbolName.trim() || composedName || undefined,
      openDate: form.openDate,
      closeDate: form.closeDate,
      entryPrice: Number(form.entryPrice),
      exitPrice: Number(form.exitPrice),
      positionSize: Number(form.positionSize),
      stopLoss: Number(form.stopLoss),
      playbookId: form.playbookId || undefined,
      tradeType: form.tradeType,
      tagIds,
      notes: form.notes.trim() || undefined,
      violatedPrincipleIds: form.violatedPrincipleIds,
      contractMultiplier: isOption
        ? Number(form.contractMultiplier) || 100
        : undefined,
    };

    try {
      await createTrade.mutateAsync(input);
      setOpen(false);
    } catch (submissionError) {
      setError(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to create trade",
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      {controlledOpen === undefined ? (
        <DialogTrigger asChild>
          {trigger ?? (
            <Button size="sm" disabled={!activeAccount}>
              <HugeiconsIcon
                icon={PlusSignIcon}
                strokeWidth={2}
                className="size-4"
              />
              Enter Trade
            </Button>
          )}
        </DialogTrigger>
      ) : null}
      <DialogContent className="flex max-h-[calc(100svh-2rem)] flex-col overflow-hidden sm:max-w-3xl">
        <form
          onSubmit={handleSubmit}
          className="grid min-h-0 flex-1 grid-rows-[auto_1fr_auto] gap-4 overflow-hidden"
        >
          <DialogHeader className="shrink-0">
            <DialogTitle className="flex items-center gap-2">
              <span className="flex size-7 items-center justify-center rounded-md bg-muted text-muted-foreground">
                <HugeiconsIcon
                  icon={PlusSignIcon}
                  strokeWidth={2}
                  className="size-4"
                />
              </span>
              Enter trade
            </DialogTitle>
            <DialogDescription>
              Add a journal entry with the trade details for{" "}
              {activeAccount?.name ?? "the selected account"}.
            </DialogDescription>
          </DialogHeader>

          <ScrollArea className="-mx-6 min-h-0 px-6 [&>[data-radix-scroll-area-viewport]]:max-h-[60svh]">
            <div className="grid gap-4 py-4 md:grid-cols-2">
              <Field label="Instrument" className="md:col-span-2">
                <Select
                  value={form.instrument}
                  onValueChange={(value) =>
                    setField("instrument", value as Instrument)
                  }
                >
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="stock">Stock</SelectItem>
                    <SelectItem value="option">Option</SelectItem>
                  </SelectContent>
                </Select>
              </Field>

              <Field
                label={isOption ? "Underlying" : "Symbol"}
                htmlFor="trade-symbol"
              >
                <Input
                  id="trade-symbol"
                  value={form.symbol}
                  onChange={(event) => setField("symbol", event.target.value)}
                  placeholder="AAPL"
                />
              </Field>

              <Field label="Symbol Name" htmlFor="trade-symbol-name">
                <Input
                  id="trade-symbol-name"
                  value={form.symbolName}
                  onChange={(event) =>
                    setField("symbolName", event.target.value)
                  }
                  placeholder={
                    isOption
                      ? "Auto-built from the contract below"
                      : "Optional, auto-fetched if omitted"
                  }
                />
              </Field>

              {isOption ? (
                <>
                  <Field label="Option Type">
                    <Select
                      value={form.optionKind}
                      onValueChange={(value) =>
                        setField("optionKind", value as OptionKind)
                      }
                    >
                      <SelectTrigger>
                        <SelectValue />
                      </SelectTrigger>
                      <SelectContent>
                        <SelectItem value="CALL">Call</SelectItem>
                        <SelectItem value="PUT">Put</SelectItem>
                      </SelectContent>
                    </Select>
                  </Field>

                  <Field label="Strike" htmlFor="trade-strike">
                    <Input
                      id="trade-strike"
                      type="number"
                      step="0.01"
                      min="0"
                      value={form.strike}
                      onChange={(event) =>
                        setField("strike", event.target.value)
                      }
                      placeholder="150"
                    />
                  </Field>

                  <Field label="Expiration" htmlFor="trade-expiration">
                    <Input
                      id="trade-expiration"
                      type="date"
                      value={form.expiration}
                      onChange={(event) =>
                        setField("expiration", event.target.value)
                      }
                    />
                  </Field>

                  <Field label="Contract Multiplier" htmlFor="trade-multiplier">
                    <Input
                      id="trade-multiplier"
                      type="number"
                      step="1"
                      min="1"
                      value={form.contractMultiplier}
                      onChange={(event) =>
                        setField("contractMultiplier", event.target.value)
                      }
                      placeholder="100"
                    />
                  </Field>
                </>
              ) : null}

              <Field label="Open Date" htmlFor="trade-open-date">
                <DateTimePicker
                  id="trade-open-date"
                  value={form.openDate}
                  onChange={(value) => setField("openDate", value)}
                />
              </Field>

              <Field label="Close Date" htmlFor="trade-close-date">
                <DateTimePicker
                  id="trade-close-date"
                  value={form.closeDate}
                  onChange={(value) => setField("closeDate", value)}
                />
              </Field>

              <Field label="Entry Price" htmlFor="trade-entry-price">
                <Input
                  id="trade-entry-price"
                  type="number"
                  step="0.0001"
                  min="0"
                  value={form.entryPrice}
                  onChange={(event) =>
                    setField("entryPrice", event.target.value)
                  }
                  placeholder="0.00"
                />
              </Field>

              <Field label="Exit Price" htmlFor="trade-exit-price">
                <Input
                  id="trade-exit-price"
                  type="number"
                  step="0.0001"
                  min="0"
                  value={form.exitPrice}
                  onChange={(event) =>
                    setField("exitPrice", event.target.value)
                  }
                  placeholder="0.00"
                />
              </Field>

              <Field
                label={isOption ? "Position Size (contracts)" : "Position Size"}
                htmlFor="trade-position-size"
              >
                <Input
                  id="trade-position-size"
                  type="number"
                  step="0.01"
                  min="0"
                  value={form.positionSize}
                  onChange={(event) =>
                    setField("positionSize", event.target.value)
                  }
                  placeholder={isOption ? "1" : "1000.00"}
                />
              </Field>

              <Field label="Stop Loss" htmlFor="trade-stop-loss">
                <Input
                  id="trade-stop-loss"
                  type="number"
                  step="0.0001"
                  min="0"
                  value={form.stopLoss}
                  onChange={(event) => setField("stopLoss", event.target.value)}
                  placeholder="0.00"
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
                  value={form.playbookId || "__none__"}
                  onValueChange={(value) =>
                    setField("playbookId", value === "__none__" ? "" : value)
                  }
                  disabled={playbooks.isLoading}
                >
                  <SelectTrigger>
                    <SelectValue placeholder="No playbook" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__none__">No playbook</SelectItem>
                    {playbooks.data?.map((playbook) => (
                      <SelectItem key={playbook.id} value={playbook.id}>
                        {playbook.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </Field>

              <Field label="Notes" htmlFor="trade-notes">
                <textarea
                  id="trade-notes"
                  value={form.notes}
                  onChange={(event) => setField("notes", event.target.value)}
                  placeholder="Optional notes"
                  rows={4}
                  className={cn(
                    "min-h-24 w-full rounded-md border border-input bg-input/20 px-3 py-2 text-sm outline-none transition-colors placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30",
                  )}
                />
              </Field>

              <Field label="Principles broken" className="self-start">
                <PrinciplePicker
                  accountId={activeAccountId}
                  selectedPlaybookId={selectedPlaybookId}
                  value={form.violatedPrincipleIds}
                  onChange={(ids) => setField("violatedPrincipleIds", ids)}
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
          </ScrollArea>

          <DialogFooter className="shrink-0">
            <Button type="submit" disabled={createTrade.isPending}>
              {createTrade.isPending ? "Saving..." : "Save Trade"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
