import { useCallback, useEffect, useState } from "react";
import { TrashIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { notify } from "../../user-interface/toast";
import {
  deletePositionCalculatorHistory,
  positionCalculatorHistory,
  type PositionCalculatorHistoryEntry,
} from "../../../backend";
import { fmt } from "./formulas";

export function HistoryPanel() {
  const [data, setData] = useState<PositionCalculatorHistoryEntry[] | null>(
    null,
  );
  const [state, setState] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    setState("loading");
    positionCalculatorHistory()
      .then((entries) => {
        setData(entries);
        setState("ready");
      })
      .catch((e) => {
        setError(String(e));
        setState("error");
      });
  }, []);

  useEffect(() => {
    reload();
  }, [reload]);

  function handleDelete(id: string) {
    deletePositionCalculatorHistory(id)
      .then(() => {
        notify.success("History entry deleted.");
        reload();
      })
      .catch((e) => notify.error("Failed to delete history entry.", String(e)));
  }

  if (state === "loading" && !data) {
    return (
      <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
    );
  }

  if (state === "error") {
    return (
      <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
        Couldn't load history: {error}
      </p>
    );
  }

  if (!data || data.length === 0) {
    return (
      <div className="flex flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-zinc-300 p-12 text-center dark:border-zinc-700">
        <p className="text-sm font-medium text-zinc-700 dark:text-zinc-200">
          No calculations saved yet
        </p>
        <p className="text-sm text-zinc-500 dark:text-zinc-400">
          Use "Save to history" in the Calculator tab.
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-2xl border border-zinc-200/80 bg-white/85 p-2 shadow-sm backdrop-blur-md dark:border-zinc-800 dark:bg-zinc-900/70">
      <Table>
        <TableHeader>
          <TableRow>
            <TableHead>Symbol</TableHead>
            <TableHead>Type</TableHead>
            <TableHead className="text-right">Entry</TableHead>
            <TableHead className="text-right">Stop</TableHead>
            <TableHead className="text-right">Shares</TableHead>
            <TableHead className="text-right">Value</TableHead>
            <TableHead className="text-right">% Acct</TableHead>
            <TableHead />
          </TableRow>
        </TableHeader>
        <TableBody>
          {data.map((entry) => (
            <TableRow key={entry.id}>
              <TableCell className="font-medium">{entry.symbol}</TableCell>
              <TableCell className="capitalize">
                {entry.positionType}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                ${fmt(entry.entryPrice)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                ${fmt(entry.stopLoss)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {fmt(entry.shares)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                ${fmt(entry.positionValue)}
              </TableCell>
              <TableCell className="text-right tabular-nums">
                {fmt(entry.accountPct)}%
              </TableCell>
              <TableCell>
                <Button
                  variant="ghost"
                  size="icon-sm"
                  aria-label="Delete history entry"
                  className="text-muted-foreground hover:text-destructive"
                  onClick={() => handleDelete(entry.id)}
                >
                  <TrashIcon size={15} />
                </Button>
              </TableCell>
            </TableRow>
          ))}
        </TableBody>
      </Table>
    </div>
  );
}
