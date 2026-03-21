"use client";

import { useEffect, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useGraphQLSubscription } from "@/lib/client";
import { AI_JOB_EVENTS_SUBSCRIPTION } from "@/lib/service/ai";
import type {
  AiArtifactEnvelope,
  AiArtifactKind,
  AiJobEvent,
} from "@/lib/types/ai";
import type { AnalyticsRange, AnalyticsTimeFilterInput } from "@/lib/types/analytics";
import { useAiArtifact, useAiJobMutation, aiQueryKey } from "@/hooks/ai";
import {
  useAccountsError,
  useAccountsLoading,
  useActiveAccount,
} from "@/components/accounts/hooks";
import { Button } from "@/components/ui/button";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyTitle,
} from "@/components/ui/empty";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const RANGE_OPTIONS: Array<{ label: string; value: AnalyticsRange }> = [
  { label: "Last 7 days", value: "LAST_7_DAYS" },
  { label: "Last 30 days", value: "LAST_30_DAYS" },
  { label: "Year to date", value: "YEAR_TO_DATE" },
  { label: "Last year", value: "LAST_1_YEAR" },
];

function buildTimeFilter(range: AnalyticsRange): AnalyticsTimeFilterInput {
  return { range };
}

function ArtifactContent({
  artifact,
  kind,
}: {
  artifact: AiArtifactEnvelope;
  kind: AiArtifactKind;
}) {
  if (kind === "insights" && artifact.insightBundle) {
    return (
      <div className="space-y-4">
        <div className="rounded-xl border bg-muted/30 p-4">
          <p className="text-sm text-muted-foreground">
            {artifact.insightBundle.overview}
          </p>
        </div>
        <div className="grid gap-4 lg:grid-cols-2">
          {artifact.insightBundle.cards.map((card) => (
            <article key={card.title} className="rounded-xl border bg-background p-4">
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-medium">{card.title}</h3>
                <span className="rounded-full border px-2 py-0.5 text-[0.65rem] uppercase tracking-[0.16em] text-muted-foreground">
                  {card.severity}
                </span>
              </div>
              <p className="mt-3 text-sm text-muted-foreground">{card.summary}</p>
              <p className="mt-3 text-[0.7rem] uppercase tracking-[0.18em] text-muted-foreground">
                {card.category}
              </p>
              <div className="mt-3 flex flex-wrap gap-2">
                {card.citations.map((citation) => (
                  <span
                    key={`${citation.sourceType}-${citation.sourceId}-${citation.title}`}
                    className="rounded-md border px-2 py-1 text-[0.7rem] text-muted-foreground"
                  >
                    {citation.title}
                  </span>
                ))}
              </div>
            </article>
          ))}
        </div>
        <div className="rounded-xl border bg-background p-4">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
            Next Actions
          </p>
          <div className="mt-3 grid gap-2">
            {artifact.insightBundle.nextActions.map((item) => (
              <p key={item} className="text-sm text-muted-foreground">
                {item}
              </p>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (kind === "report" && artifact.report) {
    return (
      <div className="space-y-4">
        <div className="rounded-xl border bg-background p-5">
          <h2 className="text-lg font-semibold">{artifact.report.title}</h2>
          <p className="mt-2 text-sm text-muted-foreground">
            {artifact.report.summary}
          </p>
        </div>
        <div className="grid gap-4">
          {artifact.report.sections.map((section) => (
            <article key={section.heading} className="rounded-xl border bg-background p-4">
              <h3 className="text-sm font-medium">{section.heading}</h3>
              <p className="mt-3 text-sm text-muted-foreground">{section.body}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                {section.citations.map((citation) => (
                  <span
                    key={`${citation.sourceType}-${citation.sourceId}-${citation.title}`}
                    className="rounded-md border px-2 py-1 text-[0.7rem] text-muted-foreground"
                  >
                    {citation.title}
                  </span>
                ))}
              </div>
            </article>
          ))}
        </div>
        <div className="rounded-xl border bg-muted/30 p-4">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
            Next Actions
          </p>
          <div className="mt-3 grid gap-2">
            {artifact.report.nextActions.map((item) => (
              <p key={item} className="text-sm text-muted-foreground">
                {item}
              </p>
            ))}
          </div>
        </div>
      </div>
    );
  }

  if (kind === "mindset" && artifact.mindsetSummary) {
    return (
      <div className="space-y-4">
        <div className="rounded-xl border bg-muted/30 p-4">
          <p className="text-sm text-muted-foreground">
            {artifact.mindsetSummary.overview}
          </p>
        </div>
        <div className="grid gap-4 lg:grid-cols-2">
          {artifact.mindsetSummary.signals.map((signal) => (
            <article key={signal.pattern} className="rounded-xl border bg-background p-4">
              <h3 className="text-sm font-medium">{signal.pattern}</h3>
              <p className="mt-3 text-sm text-muted-foreground">{signal.evidence}</p>
              <p className="mt-3 text-sm">{signal.coaching}</p>
              <div className="mt-3 flex flex-wrap gap-2">
                {signal.citations.map((citation) => (
                  <span
                    key={`${citation.sourceType}-${citation.sourceId}-${citation.title}`}
                    className="rounded-md border px-2 py-1 text-[0.7rem] text-muted-foreground"
                  >
                    {citation.title}
                  </span>
                ))}
              </div>
            </article>
          ))}
        </div>
        <div className="rounded-xl border bg-background p-4">
          <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
            Suggested Routines
          </p>
          <div className="mt-3 grid gap-2">
            {artifact.mindsetSummary.routines.map((item) => (
              <p key={item} className="text-sm text-muted-foreground">
                {item}
              </p>
            ))}
          </div>
        </div>
      </div>
    );
  }

  return null;
}

export function AIWorkspace({
  kind,
  title,
  description,
  actionLabel,
}: {
  kind: AiArtifactKind;
  title: string;
  description: string;
  actionLabel: string;
}) {
  const account = useActiveAccount();
  const accountsLoading = useAccountsLoading();
  const accountsError = useAccountsError();
  const subscribe = useGraphQLSubscription();
  const queryClient = useQueryClient();
  const [range, setRange] = useState<AnalyticsRange>("LAST_30_DAYS");
  const [liveEvent, setLiveEvent] = useState<AiJobEvent | null>(null);
  const [activeJobId, setActiveJobId] = useState<string | null>(null);

  const request = account
    ? {
        accountId: account.id,
        timeFilter: buildTimeFilter(range),
      }
    : null;

  const artifactQuery = useAiArtifact(kind, request);
  const mutation = useAiJobMutation(kind);

  useEffect(() => {
    setLiveEvent(null);
    setActiveJobId(null);
  }, [account?.id, range]);

  useEffect(() => {
    if (!activeJobId || !request) {
      return;
    }

    return subscribe<{ aiJobEvents: AiJobEvent }>(
      AI_JOB_EVENTS_SUBSCRIPTION,
      { jobId: activeJobId },
      {
        onMessage: (data) => {
          const event = data.aiJobEvents;
          setLiveEvent(event);
          if (event.status === "completed") {
            queryClient.invalidateQueries({
              queryKey: aiQueryKey(kind, request),
            });
            setActiveJobId(null);
          }
          if (event.status === "failed") {
            setActiveJobId(null);
          }
        },
        onError: (error) => {
          setLiveEvent({
            jobId: activeJobId,
            accountId: request.accountId,
            artifactType: null,
            status: "failed",
            message: null,
            artifact: null,
            error: error.message,
          });
          setActiveJobId(null);
        },
      },
    );
  }, [activeJobId, kind, queryClient, request, subscribe]);

  const artifact = liveEvent?.artifact ?? artifactQuery.data ?? null;

  return (
    <div className="grid gap-4">
      <section className="grid gap-4 xl:grid-cols-[1.5fr_1fr]">
        <div className="rounded-2xl border bg-background p-5">
          <p className="text-[0.7rem] font-medium uppercase tracking-[0.22em] text-muted-foreground">
            {title}
          </p>
          <h2 className="mt-2 text-xl font-semibold tracking-tight">
            {account ? account.name : "No account selected"}
          </h2>
          <p className="mt-2 max-w-2xl text-sm text-muted-foreground">
            {description}
          </p>
          {liveEvent ? (
            <div className="mt-4 rounded-xl border bg-muted/30 p-4">
              <p className="text-xs font-medium uppercase tracking-[0.18em] text-muted-foreground">
                Live Status
              </p>
              <p className="mt-2 text-sm capitalize">{liveEvent.status}</p>
              <p className="mt-1 text-sm text-muted-foreground">
                {liveEvent.message ?? liveEvent.error ?? "Streaming update"}
              </p>
            </div>
          ) : null}
        </div>

        <div className="rounded-2xl border bg-background p-5">
          <p className="text-[0.7rem] font-medium uppercase tracking-[0.22em] text-muted-foreground">
            Controls
          </p>
          <div className="mt-4 grid gap-3">
            <Select value={range} onValueChange={(value) => setRange(value as AnalyticsRange)}>
              <SelectTrigger className="w-full justify-between">
                <SelectValue placeholder="Select time range" />
              </SelectTrigger>
              <SelectContent>
                {RANGE_OPTIONS.map((option) => (
                  <SelectItem key={option.value} value={option.value}>
                    {option.label}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Button
              size="lg"
              onClick={() => {
                if (!request) return;
                mutation.mutate(request, {
                  onSuccess: (handle) => {
                    setLiveEvent(null);
                    setActiveJobId(handle.jobId);
                  },
                });
              }}
              disabled={!request || mutation.isPending || !!activeJobId}
            >
              {activeJobId ? "Streaming..." : actionLabel}
            </Button>
          </div>
          <div className="mt-4 text-xs text-muted-foreground">
            {accountsLoading
              ? "Loading accounts..."
              : accountsError
                ? accountsError
                : account
                  ? `Scoped to ${account.name}`
                  : "Create or select an account to use AI features."}
          </div>
        </div>
      </section>

      {artifact ? (
        <ArtifactContent artifact={artifact} kind={kind} />
      ) : (
        <Empty className="min-h-[18rem] rounded-2xl border bg-background">
          <EmptyHeader>
            <EmptyTitle>No AI output yet</EmptyTitle>
            <EmptyDescription>
              Generate a fresh {title.toLowerCase()} artifact for the currently
              selected account. The result will stream back live as the backend
              worker finishes retrieval and generation.
            </EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            {artifactQuery.isLoading || mutation.isPending
              ? "Preparing the workspace..."
              : "Start a run to populate this page."}
          </EmptyContent>
        </Empty>
      )}
    </div>
  );
}
