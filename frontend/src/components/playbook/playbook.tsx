"use client";

import {
  Delete02Icon,
  PencilEdit01Icon,
  PlusSignIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useDeletePlaybook, usePlaybooks } from "@/hooks/playbook";
import { CreatePlaybookDialog } from "./create-playbook";
import { EditPlaybookDialog } from "./edit-playbook";
import { PrinciplesTab } from "./principles-tab";

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
                    <div className="grid gap-1 text-xs text-muted-foreground">
                      <p>
                        <span className="font-medium text-foreground">
                          Entry:
                        </span>{" "}
                        {playbook.entryRules}
                      </p>
                      <p>
                        <span className="font-medium text-foreground">
                          Exit:
                        </span>{" "}
                        {playbook.exitRules}
                      </p>
                      <p>
                        <span className="font-medium text-foreground">
                          Position sizing:
                        </span>{" "}
                        {playbook.positionSizingRules}
                      </p>
                      {playbook.additionalRules ? (
                        <p>
                          <span className="font-medium text-foreground">
                            Additional:
                          </span>{" "}
                          {playbook.additionalRules}
                        </p>
                      ) : null}
                    </div>

                    <div className="grid grid-cols-2 gap-2">
                      <div className="rounded-md border bg-muted/30 p-2">
                        <p className="text-[11px] text-muted-foreground">
                          Win rate
                        </p>
                        <p className="text-sm font-semibold">
                          {formatPercent(playbook.winRate)}
                        </p>
                      </div>
                      <div className="rounded-md border bg-muted/30 p-2">
                        <p className="text-[11px] text-muted-foreground">
                          Cum. profit
                        </p>
                        <p className="text-sm font-semibold">
                          {formatUsd(playbook.cumulativeProfit)}
                        </p>
                      </div>
                      <div className="rounded-md border bg-muted/30 p-2">
                        <p className="text-[11px] text-muted-foreground">
                          Avg gain
                        </p>
                        <p className="text-sm font-semibold">
                          {formatUsd(playbook.averageGain)}
                        </p>
                      </div>
                      <div className="rounded-md border bg-muted/30 p-2">
                        <p className="text-[11px] text-muted-foreground">
                          Avg loss
                        </p>
                        <p className="text-sm font-semibold">
                          {formatUsd(playbook.averageLoss)}
                        </p>
                      </div>
                    </div>

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
