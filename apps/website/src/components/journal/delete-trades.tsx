"use client";

import { Delete02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
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
import { useDeleteJournalEntry } from "@/hooks/journal";
import type { JournalEntry } from "@/lib/types/journal";

interface DeleteTradesProps {
  trade: JournalEntry;
}

export function DeleteTrades({ trade }: DeleteTradesProps) {
  const deleteTrade = useDeleteJournalEntry();
  const [open, setOpen] = React.useState(false);
  const [error, setError] = React.useState("");

  async function handleDelete() {
    try {
      await deleteTrade.mutateAsync(trade.id);
      setOpen(false);
      setError("");
    } catch (submissionError) {
      setError(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to delete trade",
      );
    }
  }

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          className="text-muted-foreground hover:bg-destructive/10 hover:text-destructive"
        >
          <HugeiconsIcon
            icon={Delete02Icon}
            strokeWidth={2}
            className="size-4"
          />
          <span className="sr-only">Delete trade</span>
        </Button>
      </DialogTrigger>
      <DialogContent className="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>Delete trade</DialogTitle>
          <DialogDescription>
            This will permanently remove the trade for {trade.symbol}. This
            action cannot be undone.
          </DialogDescription>
        </DialogHeader>

        <div className="rounded-lg border bg-muted/30 p-3 text-sm text-muted-foreground">
          <p className="font-medium text-foreground">{trade.symbol}</p>
          <p className="mt-1">{trade.mistakes}</p>
        </div>

        {error ? <p className="text-sm text-destructive">{error}</p> : null}

        <DialogFooter>
          <Button variant="outline" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            onClick={handleDelete}
            disabled={deleteTrade.isPending}
          >
            {deleteTrade.isPending ? "Deleting..." : "Delete Trade"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
