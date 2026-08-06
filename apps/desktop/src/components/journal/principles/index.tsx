import { useCallback, useEffect, useState } from "react";
import {
  CaretDownIcon,
  CaretUpIcon,
  PencilSimpleIcon,
  PlusIcon,
  ToggleLeftIcon,
  ToggleRightIcon,
  TrashIcon,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  accounts,
  deletePrinciple,
  principles as fetchPrinciples,
  reorderPrinciples,
  updatePrinciple,
  type PrincipleWithStats,
} from "../../../backend";
import { PrincipleFormDialog } from "./principle-form";
import { Stagger, StaggerItem } from "../../user-interface";

const usd = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
});
const signedUsd = (v: number) => `${v >= 0 ? "+" : ""}${usd.format(v)}`;

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
          aria-label="Delete principle"
          className="text-muted-foreground hover:text-destructive"
        >
          <TrashIcon size={15} />
        </Button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-64 gap-3">
        <div>
          <p className="text-sm font-semibold text-foreground">
            Delete principle?
          </p>
          <p className="mt-1 text-xs text-muted-foreground">
            This permanently deletes {name} and its violation history.
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

function PrincipleCard({
  principle: p,
  accountId,
  isFirst,
  isLast,
  onChanged,
  onDelete,
  onMove,
}: {
  principle: PrincipleWithStats;
  accountId: string;
  isFirst: boolean;
  isLast: boolean;
  onChanged: () => void;
  onDelete: (id: string) => void;
  onMove: (id: string, direction: "up" | "down") => void;
}) {
  const negative = p.violatedCumulativeProfit < 0;
  const untouched = p.violationCount === 0;

  return (
    <article className="group flex flex-col gap-4 rounded-2xl border border-zinc-200/80 bg-white/85 p-5 shadow-sm backdrop-blur-md transition duration-200 hover:border-zinc-300 hover:shadow-md dark:border-zinc-800 dark:bg-zinc-900/70 dark:hover:border-zinc-700">
      <div className="flex items-start justify-between gap-2">
        <div className="flex min-w-0 items-start gap-1">
          <div className="flex flex-col pt-0.5">
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Move up"
              disabled={isFirst}
              onClick={() => onMove(p.id, "up")}
              className="h-5 w-5 text-muted-foreground disabled:opacity-30"
            >
              <CaretUpIcon size={13} />
            </Button>
            <Button
              variant="ghost"
              size="icon-sm"
              aria-label="Move down"
              disabled={isLast}
              onClick={() => onMove(p.id, "down")}
              className="h-5 w-5 text-muted-foreground disabled:opacity-30"
            >
              <CaretDownIcon size={13} />
            </Button>
          </div>
          <div className="min-w-0">
            <h3 className="truncate text-[15px] font-semibold text-zinc-900 dark:text-zinc-50">
              {p.title}
            </h3>
            <span className="mt-1.5 inline-flex items-center rounded-full bg-zinc-100 px-2 py-0.5 text-[11px] font-medium text-zinc-600 dark:bg-zinc-800 dark:text-zinc-300">
              {p.playbookId ? "Playbook-specific" : "Applies to every trade"}
            </span>
            {!p.isActive && (
              <span className="ml-1.5 mt-1.5 inline-flex items-center rounded-full bg-amber-100 px-2 py-0.5 text-[11px] font-medium text-amber-700 dark:bg-amber-900/40 dark:text-amber-400">
                Inactive
              </span>
            )}
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-0.5 opacity-0 transition-opacity duration-150 group-hover:opacity-100 focus-within:opacity-100">
          <Button
            variant="ghost"
            size="icon-sm"
            aria-label={p.isActive ? "Deactivate" : "Activate"}
            onClick={() =>
              updatePrinciple(p.id, { isActive: !p.isActive }).then(onChanged)
            }
            className="text-muted-foreground"
          >
            {p.isActive ? (
              <ToggleRightIcon size={17} weight="fill" />
            ) : (
              <ToggleLeftIcon size={17} />
            )}
          </Button>
          <PrincipleFormDialog
            accountId={accountId}
            principle={p}
            onSaved={onChanged}
            trigger={
              <Button
                variant="ghost"
                size="icon-sm"
                aria-label="Edit principle"
              >
                <PencilSimpleIcon size={15} />
              </Button>
            }
          />
          <DeleteButton name={p.title} onConfirm={() => onDelete(p.id)} />
        </div>
      </div>

      <div>
        <p
          className={`text-2xl font-semibold tabular-nums ${
            untouched
              ? "text-zinc-400 dark:text-zinc-600"
              : negative
                ? "text-red-600 dark:text-red-400"
                : "text-emerald-600 dark:text-emerald-400"
          }`}
        >
          {signedUsd(p.violatedCumulativeProfit)}
        </p>
        <p className="mt-0.5 text-xs text-zinc-500 dark:text-zinc-400">
          <span className="font-medium tabular-nums text-zinc-700 dark:text-zinc-200">
            {p.violatedWinRate.toFixed(1)}%
          </span>{" "}
          win rate on violations · {p.violationCount} break
          {p.violationCount === 1 ? "" : "s"}
        </p>
      </div>

      <dl className="flex flex-col gap-1.5 border-t border-zinc-100 pt-3 dark:border-zinc-800/70">
        <div className="grid grid-cols-[4.5rem_1fr] gap-2">
          <dt className="text-xs font-medium text-zinc-400 dark:text-zinc-500">
            Rule
          </dt>
          <dd className="text-xs leading-relaxed text-zinc-600 dark:text-zinc-300">
            {p.theRule}
          </dd>
        </div>
        <div className="grid grid-cols-[4.5rem_1fr] gap-2">
          <dt className="text-xs font-medium text-zinc-400 dark:text-zinc-500">
            Why
          </dt>
          <dd className="text-xs leading-relaxed text-zinc-600 dark:text-zinc-300">
            {p.why}
          </dd>
        </div>
        {p.intervention && (
          <div className="grid grid-cols-[4.5rem_1fr] gap-2">
            <dt className="text-xs font-medium text-zinc-400 dark:text-zinc-500">
              Fix
            </dt>
            <dd className="text-xs leading-relaxed text-zinc-600 dark:text-zinc-300">
              {p.intervention}
            </dd>
          </div>
        )}
      </dl>
    </article>
  );
}

export default function PrinciplesView() {
  // undefined = resolving, null = no account, string = active account id
  const [accountId, setAccountId] = useState<string | null | undefined>(
    undefined,
  );
  const [data, setData] = useState<PrincipleWithStats[] | null>(null);
  const [state, setState] = useState<"loading" | "ready" | "error">(
    "loading",
  );
  const [error, setError] = useState<string | null>(null);

  const reload = useCallback((id: string) => {
    setState("loading");
    fetchPrinciples(id)
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
    accounts()
      .then((accs) => setAccountId(accs[0]?.id ?? null))
      .catch(() => setAccountId(null));
  }, []);

  useEffect(() => {
    if (accountId) reload(accountId);
  }, [accountId, reload]);

  const handleDelete = (id: string) => {
    if (!accountId) return;
    deletePrinciple(id)
      .then(() => reload(accountId))
      .catch((e) => setError(String(e)));
  };

  const handleMove = (id: string, direction: "up" | "down") => {
    if (!accountId || !data) return;
    const index = data.findIndex((p) => p.id === id);
    const swapWith = direction === "up" ? index - 1 : index + 1;
    if (index < 0 || swapWith < 0 || swapWith >= data.length) return;
    const next = [...data];
    [next[index], next[swapWith]] = [next[swapWith], next[index]];
    setData(next);
    reorderPrinciples(next.map((p) => p.id))
      .then(() => reload(accountId))
      .catch((e) => {
        setError(String(e));
        reload(accountId);
      });
  };

  if (accountId === undefined) {
    return (
      <p className="text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
    );
  }

  if (accountId === null) {
    return (
      <p className="text-sm text-zinc-500 dark:text-zinc-400">
        No account yet — connect a brokerage to track principles.
      </p>
    );
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="flex justify-end">
        <PrincipleFormDialog
          accountId={accountId}
          onSaved={() => reload(accountId)}
          trigger={
            <Button size="sm">
              <PlusIcon size={15} weight="bold" />
              New principle
            </Button>
          }
        />
      </div>

      {state === "loading" && !data && (
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
          Couldn't load principles: {error}
        </p>
      )}

      {data && data.length === 0 && (
        <div className="flex flex-col items-center justify-center gap-1 rounded-2xl border border-dashed border-zinc-300 p-12 text-center dark:border-zinc-700">
          <p className="text-sm font-medium text-zinc-700 dark:text-zinc-200">
            No principles yet
          </p>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">
            Write the rules you keep breaking. You'll see what each one costs
            you.
          </p>
        </div>
      )}

      {data && data.length > 0 && (
        <Stagger className="grid gap-4 sm:grid-cols-2 xl:grid-cols-3">
          {data.map((p, i) => (
            <StaggerItem key={p.id}>
              <PrincipleCard
                principle={p}
                accountId={accountId}
                isFirst={i === 0}
                isLast={i === data.length - 1}
                onChanged={() => reload(accountId)}
                onDelete={handleDelete}
                onMove={handleMove}
              />
            </StaggerItem>
          ))}
        </Stagger>
      )}
    </div>
  );
}
