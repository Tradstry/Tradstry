import { useCallback, useEffect, useState } from "react";
import { PencilSimpleIcon, PlusIcon, TrashIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  deletePlaybook,
  playbooks as fetchPlaybooks,
  type Playbook,
} from "../../backend";
import { PlaybookFormDialog } from "./playbook-form";
import { Stagger, StaggerItem } from "../user-interface";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});
const signedUsd = (v: number) => `${v >= 0 ? "+" : ""}${usd.format(v)}`;

function RuleRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="grid grid-cols-[4.5rem_1fr] gap-2">
      <dt className="text-xs font-medium text-zinc-400 dark:text-zinc-500">
        {label}
      </dt>
      <dd className="text-xs leading-relaxed text-zinc-600 dark:text-zinc-300">
        {value}
      </dd>
    </div>
  );
}

function DeleteButton({
  name,
  onConfirm,
}: {
  name: string;
  onConfirm: () => void;
}) {
  const [open, setOpen] = useState(false);
  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label="Delete playbook"
          className="text-muted-foreground hover:text-destructive"
        >
          <TrashIcon size={15} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-64 gap-3">
        <div>
          <p className="text-sm font-semibold text-foreground">
            Delete playbook?
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            This permanently deletes {name} and unlinks it from trades.
          </p>
        </div>
        <div className="flex justify-end gap-2">
          <Button variant="outline" size="sm" onClick={() => setOpen(false)}>
            Cancel
          </Button>
          <Button
            size="sm"
            className="border-transparent bg-destructive text-white hover:bg-destructive/90"
            onClick={() => {
              onConfirm();
              setOpen(false);
            }}
          >
            Delete
          </Button>
        </div>
      </PopoverContent>
    </Popover>
  );
}

function PlaybookCard({
  playbook: p,
  onChanged,
  onDelete,
}: {
  playbook: Playbook;
  onChanged: () => void;
  onDelete: (id: string) => void;
}) {
  const winBar = Math.max(0, Math.min(100, p.winRate));
  const positive = p.cumulativeProfit >= 0;

  return (
    <article className="group flex flex-col gap-4 rounded-2xl border border-zinc-200/80 bg-white/85 p-5 shadow-sm backdrop-blur-md transition duration-200 hover:border-zinc-300 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900/70 dark:hover:border-zinc-700">
      {/* Header: name + edge chip, actions reveal on hover */}
      <div className="flex items-start justify-between gap-2">
        <div className="min-w-0">
          <h3 className="truncate text-[15px] font-semibold text-zinc-900 dark:text-zinc-50">
            {p.name}
          </h3>
          <span className="mt-1.5 inline-flex items-center rounded-full bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
            {p.edgeName}
          </span>
        </div>
        <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100">
          <PlaybookFormDialog
            playbook={p}
            onSaved={onChanged}
            trigger={
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Edit playbook"
              >
                <PencilSimpleIcon size={15} />
              </Button>
            }
          />
          <DeleteButton name={p.name} onConfirm={() => onDelete(p.id)} />
        </div>
      </div>

      {/* Hero metric: cumulative P/L */}
      <div>
        <p
          className={`text-2xl font-semibold tabular-nums ${positive ? "text-emerald-600 dark:text-emerald-400" : "text-red-600 dark:text-red-400"}`}
        >
          {signedUsd(p.cumulativeProfit)}
        </p>
        <p className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
          <span className="font-medium tabular-nums text-zinc-700 dark:text-zinc-200">
            {p.winRate.toFixed(1)}%
          </span>{" "}
          win rate · {p.tradeCount} trade{p.tradeCount === 1 ? "" : "s"}
        </p>
        <div className="mt-2 h-1 w-full overflow-hidden rounded-full bg-zinc-200/70 dark:bg-zinc-800">
          <div
            className="h-full rounded-full bg-emerald-500 dark:bg-emerald-400"
            style={{ width: `${winBar}%` }}
          />
        </div>
      </div>

      {/* Secondary stats */}
      <div className="flex gap-6 border-t border-zinc-100 pt-3 dark:border-zinc-800/70">
        <div>
          <p className="text-[11px] text-zinc-500 dark:text-zinc-400">
            Avg gain
          </p>
          <p className="text-sm font-semibold tabular-nums text-emerald-600 dark:text-emerald-400">
            {usd.format(p.averageGain)}
          </p>
        </div>
        <div>
          <p className="text-[11px] text-zinc-500 dark:text-zinc-400">
            Avg loss
          </p>
          <p className="text-sm font-semibold tabular-nums text-red-600 dark:text-red-400">
            {usd.format(-p.averageLoss)}
          </p>
        </div>
      </div>

      {/* Rules */}
      <dl className="flex flex-col gap-1.5 border-t border-zinc-100 pt-3 dark:border-zinc-800/70">
        <RuleRow label="Entry" value={p.entryRules} />
        <RuleRow label="Exit" value={p.exitRules} />
        <RuleRow label="Sizing" value={p.positionSizingRules} />
        {p.additionalRules && <RuleRow label="Notes" value={p.additionalRules} />}
      </dl>
    </article>
  );
}

export default function PlaybookView() {
  const [data, setData] = useState<Playbook[] | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback(() => {
    fetchPlaybooks()
      .then((p) => {
        setData(p);
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

  const handleDelete = (id: string) => {
    deletePlaybook(id)
      .then(reload)
      .catch((e) => setError(String(e)));
  };

  return (
    <div className="flex flex-col gap-4">
      <div className="flex justify-end">
        <PlaybookFormDialog
          onSaved={reload}
          trigger={
            <Button size="sm">
              <PlusIcon size={15} weight="bold" />
              New playbook
            </Button>
          }
        />
      </div>

      {state === "loading" && (
        <div className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {Array.from({ length: 3 }).map((_, i) => (
            <div
              key={i}
              className="h-64 animate-pulse rounded-2xl border border-zinc-200/80 bg-zinc-100/70 dark:border-zinc-800 dark:bg-zinc-900/50"
            />
          ))}
        </div>
      )}

      {state === "error" && (
        <p className="rounded-md bg-red-50 px-3 py-2 text-sm text-red-600 dark:bg-red-950/40 dark:text-red-400">
          Couldn't load playbooks: {error}
        </p>
      )}

      {state === "ready" && data && data.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-zinc-300 p-12 text-center dark:border-zinc-700">
          <p className="text-sm font-medium text-zinc-700 dark:text-zinc-200">
            No playbooks yet
          </p>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">
            Create your first playbook to track its rules and stats.
          </p>
        </div>
      )}

      {state === "ready" && data && data.length > 0 && (
        <Stagger className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {data.map((p) => (
            <StaggerItem key={p.id}>
              <PlaybookCard
                playbook={p}
                onChanged={reload}
                onDelete={handleDelete}
              />
            </StaggerItem>
          ))}
        </Stagger>
      )}
    </div>
  );
}
