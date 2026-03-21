"use client";

import { PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { useActiveAccount } from "@/components/accounts";
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
import { useCreateJournalEntry } from "@/hooks/journal";
import { usePlaybooks } from "@/hooks/playbook";
import type { TradeType } from "@/lib/types/journal";
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
  mistakes: string;
  entryTactics: string;
  edgesSpotted: string;
  playbookId: string;
  notes: string;
};

const initialFormState: TradeFormState = {
  symbol: "",
  symbolName: "",
  openDate: "",
  closeDate: "",
  entryPrice: "",
  exitPrice: "",
  positionSize: "",
  stopLoss: "",
  tradeType: "long",
  mistakes: "",
  entryTactics: "",
  edgesSpotted: "",
  playbookId: "",
  notes: "",
};

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
  const [internalOpen, setInternalOpen] = React.useState(false);

  const open = controlledOpen ?? internalOpen;
  const setOpen = controlledOnOpenChange ?? setInternalOpen;
  const [form, setForm] = React.useState<TradeFormState>(initialFormState);
  const [error, setError] = React.useState("");

  React.useEffect(() => {
    if (!open) {
      setForm(initialFormState);
      setError("");
    }
  }, [open]);

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
    if (!activeAccount) return "Select an account before creating a trade";
    if (!form.symbol.trim()) return "Symbol is required";
    if (!form.openDate) return "Open date is required";
    if (!form.closeDate) return "Close date is required";
    if (!form.entryPrice.trim()) return "Entry price is required";
    if (!form.exitPrice.trim()) return "Exit price is required";
    if (!form.positionSize.trim()) return "Position size is required";
    if (!form.stopLoss.trim()) return "Stop loss is required";
    if (!form.mistakes.trim()) return "Mistakes is required";
    if (!form.entryTactics.trim()) return "Entry tactics is required";
    if (!form.edgesSpotted.trim()) return "Edges spotted is required";
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

    try {
      await createTrade.mutateAsync({
        accountId: activeAccount.id,
        symbol: form.symbol.trim().toUpperCase(),
        symbolName: form.symbolName.trim() || undefined,
        openDate: form.openDate,
        closeDate: form.closeDate,
        entryPrice: Number(form.entryPrice),
        exitPrice: Number(form.exitPrice),
        positionSize: Number(form.positionSize),
        stopLoss: Number(form.stopLoss),
        playbookId: form.playbookId || undefined,
        tradeType: form.tradeType,
        mistakes: form.mistakes.trim(),
        entryTactics: form.entryTactics.trim(),
        edgesSpotted: form.edgesSpotted.trim(),
        notes: form.notes.trim() || undefined,
      });
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
      <DialogContent className="sm:max-w-3xl">
        <form onSubmit={handleSubmit}>
          <DialogHeader>
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

          <div className="grid gap-4 py-4 md:grid-cols-2">
            <Field label="Symbol" htmlFor="trade-symbol">
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
                onChange={(event) => setField("symbolName", event.target.value)}
                placeholder="Optional, auto-fetched if omitted"
              />
            </Field>

            <Field label="Open Date" htmlFor="trade-open-date">
              <Input
                id="trade-open-date"
                type="datetime-local"
                value={form.openDate}
                onChange={(event) => setField("openDate", event.target.value)}
              />
            </Field>

            <Field label="Close Date" htmlFor="trade-close-date">
              <Input
                id="trade-close-date"
                type="datetime-local"
                value={form.closeDate}
                onChange={(event) => setField("closeDate", event.target.value)}
              />
            </Field>

            <Field label="Entry Price" htmlFor="trade-entry-price">
              <Input
                id="trade-entry-price"
                type="number"
                step="0.0001"
                min="0"
                value={form.entryPrice}
                onChange={(event) => setField("entryPrice", event.target.value)}
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
                onChange={(event) => setField("exitPrice", event.target.value)}
                placeholder="0.00"
              />
            </Field>

            <Field label="Position Size" htmlFor="trade-position-size">
              <Input
                id="trade-position-size"
                type="number"
                step="0.01"
                min="0"
                value={form.positionSize}
                onChange={(event) =>
                  setField("positionSize", event.target.value)
                }
                placeholder="1000.00"
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
                  setField(
                    "playbookId",
                    value === "__none__" ? "" : value,
                  )
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

            <Field label="Mistakes" htmlFor="trade-mistakes">
              <Input
                id="trade-mistakes"
                value={form.mistakes}
                onChange={(event) => setField("mistakes", event.target.value)}
                placeholder="What went wrong?"
              />
            </Field>

            <Field label="Entry Tactics" htmlFor="trade-entry-tactics">
              <Input
                id="trade-entry-tactics"
                value={form.entryTactics}
                onChange={(event) =>
                  setField("entryTactics", event.target.value)
                }
                placeholder="Execution approach"
              />
            </Field>

            <Field label="Edges Spotted" htmlFor="trade-edges-spotted">
              <Input
                id="trade-edges-spotted"
                value={form.edgesSpotted}
                onChange={(event) =>
                  setField("edgesSpotted", event.target.value)
                }
                placeholder="Observed edge"
              />
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
          </div>

          {error ? (
            <p className="pb-3 text-sm text-destructive">{error}</p>
          ) : null}

          <DialogFooter>
            <Button type="submit" disabled={createTrade.isPending}>
              {createTrade.isPending ? "Saving..." : "Save Trade"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
