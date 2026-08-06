"use client";

import {
  Delete02Icon,
  PencilEdit01Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Button } from "@tradstry/app-ui/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@tradstry/app-ui/components/ui/empty";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@tradstry/app-ui/components/ui/popover";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@tradstry/app-ui/components/ui/tabs";
import { useDeletePlaybook, usePlaybooks } from "@tradstry/app-ui/hooks/playbook";
import { cn } from "@tradstry/app-ui/lib/utils";
import { CreatePlaybookDialog } from "./create-playbook";
import { EditPlaybookDialog } from "./edit-playbook";
import { PrinciplesTab } from "./principles-tab";
import { RulesView } from "./rules-view";

const currencyFormatter = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  minimumFractionDigits: 2,
  maximumFractionDigits: 2,
});

function formatPercent(value: number) {
  return `${value.toFixed(2)}%`;
}

function formatUsd(value: number) {
  return currencyFormatter.format(value);
}

/** One performance cell. Hairline-separated rather than four bordered boxes: the numbers
 * read as one strip, and the stat that matters is the only one carrying colour. */
function Stat({
  label,
  value,
  tone,
}: {
  label: string;
  value: string;
  tone?: "profit" | "loss";
}) {
  return (
    <div className="bg-card px-2 py-1.5">
      <dt className="text-[0.6rem] font-medium uppercase tracking-[0.1em] text-muted-foreground">
        {label}
      </dt>
      <dd
        className={cn(
          "text-xs font-semibold tabular-nums",
          tone === "profit" && "text-profit",
          tone === "loss" && "text-loss",
        )}
      >
        {value}
      </dd>
    </div>
  );
}

export function Playbook() {
  const playbooksQuery = usePlaybooks();
  const deletePlaybook = useDeletePlaybook();
  const [confirmingPlaybookId, setConfirmingPlaybookId] = React.useState<
    string | null
  >(null);

  const playbooks = playbooksQuery.data ?? [];

  async function handleDelete(id: string, name: string) {
    const toastId = toast.loading(`Deleting ${name}...`);

    try {
      await deletePlaybook.mutateAsync(id);
      toast.success("Playbook deleted.", { id: toastId });
      setConfirmingPlaybookId(null);
    } catch (submissionError) {
      toast.error(
        submissionError instanceof Error
          ? submissionError.message
          : "Failed to delete playbook.",
        { id: toastId },
      );
    }
  }

  return (
    <Tabs defaultValue="playbooks" className="space-y-6">
      <TabsList>
        <TabsTrigger value="playbooks">Playbooks</TabsTrigger>
        <TabsTrigger value="principles">Principles</TabsTrigger>
      </TabsList>

      <TabsContent value="playbooks">
        <div className="space-y-6">
          <div className="mt-10 flex justify-end">
            <CreatePlaybookDialog />
          </div>

          <section className="space-y-3">
            <div className="flex items-center justify-end gap-3">
              {playbooksQuery.isLoading ? (
                <p className="text-xs text-muted-foreground">Loading…</p>
              ) : null}
            </div>

            {playbooksQuery.isError ? (
              <p className="ml-10 text-sm text-destructive">
                Failed to load playbooks.
              </p>
            ) : null}

            {!playbooksQuery.isLoading && playbooks.length === 0 ? (
              <Empty className="rounded-lg border border-dashed">
                <EmptyHeader>
                  <EmptyMedia variant="icon">
                    <HugeiconsIcon icon={PlusSignIcon} strokeWidth={2} />
                  </EmptyMedia>
                  <EmptyTitle>No playbooks yet</EmptyTitle>
                  <EmptyDescription>
                    Create a playbook to start tracking rules and stats.
                  </EmptyDescription>
                </EmptyHeader>
                <EmptyContent>
                  <CreatePlaybookDialog
                    trigger={
                      <Button size="lg">
                        <HugeiconsIcon icon={PlusSignIcon} strokeWidth={2} />
                        Create Playbook
                      </Button>
                    }
                  />
                </EmptyContent>
              </Empty>
            ) : null}

            <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
              {playbooks.map((playbook) => (
                <article
                  key={playbook.id}
                  className="max-w-sm rounded-lg border bg-card p-3"
                >
                  <div className="space-y-2">
                    <div className="flex flex-wrap items-start justify-between gap-1">
                      <div>
                        <h3 className="text-sm font-semibold">
                          {playbook.name}
                        </h3>
                        <p className="text-xs text-muted-foreground">
                          Edge: {playbook.edgeName}
                        </p>
                      </div>
                      <div className="flex items-center gap-1">
                        <EditPlaybookDialog
                          playbook={playbook}
                          trigger={
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
                              <span className="sr-only">Edit playbook</span>
                            </Button>
                          }
                        />
                        <Popover
                          open={confirmingPlaybookId === playbook.id}
                          onOpenChange={(open) =>
                            setConfirmingPlaybookId(open ? playbook.id : null)
                          }
                        >
                          <PopoverTrigger asChild>
                            <Button
                              size="icon-sm"
                              variant="ghost"
                              className="text-muted-foreground hover:text-destructive"
                              disabled={deletePlaybook.isPending}
                            >
                              <HugeiconsIcon
                                icon={Delete02Icon}
                                strokeWidth={2}
                                className="size-4"
                              />
                              <span className="sr-only">Delete playbook</span>
                            </Button>
                          </PopoverTrigger>
                          <PopoverContent
                            align="end"
                            className="space-y-3"
                            onClick={(event) => event.stopPropagation()}
                          >
                            <div className="space-y-1">
                              <p className="text-sm font-semibold">
                                Delete playbook?
                              </p>
                              <p className="text-sm text-muted-foreground">
                                This permanently deletes {playbook.name} and
                                removes it from linked trades.
                              </p>
                            </div>
                            <div className="flex justify-end gap-2">
                              <Button
                                type="button"
                                variant="outline"
                                size="sm"
                                onClick={() => setConfirmingPlaybookId(null)}
                              >
                                Cancel
                              </Button>
                              <Button
                                type="button"
                                variant="destructive"
                                size="sm"
                                disabled={deletePlaybook.isPending}
                                onClick={() =>
                                  handleDelete(playbook.id, playbook.name)
                                }
                              >
                                {deletePlaybook.isPending
                                  ? "Deleting..."
                                  : "Delete"}
                              </Button>
                            </div>
                          </PopoverContent>
                        </Popover>
                      </div>
                    </div>
                    <dl className="grid grid-cols-4 gap-px overflow-hidden rounded-md border bg-border">
                      <Stat
                        label="Win rate"
                        value={formatPercent(playbook.winRate)}
                      />
                      <Stat
                        label="Net P&L"
                        value={formatUsd(playbook.cumulativeProfit)}
                        tone={
                          playbook.cumulativeProfit >= 0 ? "profit" : "loss"
                        }
                      />
                      <Stat
                        label="Avg gain"
                        value={formatUsd(playbook.averageGain)}
                      />
                      <Stat
                        label="Avg loss"
                        value={formatUsd(playbook.averageLoss)}
                      />
                    </dl>

                    <RulesView
                      entryRules={playbook.entryRules}
                      exitRules={playbook.exitRules}
                      positionSizingRules={playbook.positionSizingRules}
                      additionalRules={playbook.additionalRules}
                    />

                    <div className="text-xs text-muted-foreground">
                      {playbook.tradeCount} linked trade
                      {playbook.tradeCount === 1 ? "" : "s"}
                    </div>
                  </div>
                </article>
              ))}
            </div>
          </section>
        </div>
      </TabsContent>

      <TabsContent value="principles">
        <PrinciplesTab />
      </TabsContent>
    </Tabs>
  );
}
