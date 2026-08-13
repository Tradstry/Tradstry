"use client";

import {
  Alert02Icon,
  ArrowRight01Icon,
  BankIcon,
  CheckmarkCircle02Icon,
  Loading03Icon,
  MultiplicationSignCircleIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { TradstryMark } from "@tradstry/app-ui/components/logo";
import { Button } from "@tradstry/app-ui/components/ui/button";
import * as brokerageService from "@tradstry/app-ui/lib/service/brokerage";
import { cn } from "@tradstry/app-ui/lib/utils";
import { useRouter, useSearchParams } from "next/navigation";
import { Suspense, useEffect, useRef, useState } from "react";
import { GraphQLProvider, useGraphQL } from "@/lib/client";

type ConnectionState =
  | { kind: "connecting" }
  | { kind: "success" }
  | { kind: "error"; detail: string }
  | { kind: "cancelled" };

type StepStatus = "complete" | "active" | "pending" | "error" | "cancelled";

const COPY = {
  connecting: {
    eyebrow: "Secure brokerage link",
    title: "Confirming your connection",
    description:
      "Tradstry is verifying the authorization and attaching it to this workspace.",
  },
  success: {
    eyebrow: "Connection confirmed",
    title: "Your brokerage is connected",
    description:
      "The secure link is ready. Next, review the accounts found under this brokerage.",
  },
  error: {
    eyebrow: "Connection needs attention",
    title: "We could not save the connection",
    description:
      "The brokerage connection did not finish, so Tradstry could not verify and attach it.",
  },
  cancelled: {
    eyebrow: "Connection cancelled",
    title: "No brokerage was connected",
    description:
      "Nothing was changed. You can return to Brokerage and start again whenever you are ready.",
  },
} as const;

function stepsFor(state: ConnectionState): Array<{
  label: string;
  detail: string;
  status: StepStatus;
}> {
  if (state.kind === "success") {
    return [
      { label: "Authorization returned", detail: "Done", status: "complete" },
      { label: "Connection verified", detail: "Done", status: "complete" },
      { label: "Workspace ready", detail: "Done", status: "complete" },
    ];
  }

  if (state.kind === "error") {
    return [
      { label: "Authorization returned", detail: "Done", status: "complete" },
      { label: "Connection verified", detail: "Blocked", status: "error" },
      { label: "Workspace ready", detail: "Waiting", status: "pending" },
    ];
  }

  if (state.kind === "cancelled") {
    return [
      {
        label: "Authorization returned",
        detail: "Cancelled",
        status: "cancelled",
      },
      {
        label: "Connection verified",
        detail: "Not started",
        status: "pending",
      },
      { label: "Workspace ready", detail: "Not started", status: "pending" },
    ];
  }

  return [
    { label: "Authorization returned", detail: "Done", status: "complete" },
    { label: "Connection verified", detail: "Checking", status: "active" },
    { label: "Workspace ready", detail: "Waiting", status: "pending" },
  ];
}

function StepMark({ status }: { status: StepStatus }) {
  if (status === "complete") {
    return (
      <span className="flex size-7 items-center justify-center rounded-full bg-profit/10 text-profit">
        <HugeiconsIcon
          icon={CheckmarkCircle02Icon}
          strokeWidth={2}
          className="size-4"
        />
      </span>
    );
  }

  if (status === "active") {
    return (
      <span className="flex size-7 items-center justify-center rounded-full bg-foreground text-background shadow-sm">
        <HugeiconsIcon
          icon={Loading03Icon}
          strokeWidth={2}
          className="size-4 animate-spin motion-reduce:animate-none"
        />
      </span>
    );
  }

  if (status === "error") {
    return (
      <span className="flex size-7 items-center justify-center rounded-full bg-loss/10 text-loss">
        <HugeiconsIcon
          icon={MultiplicationSignCircleIcon}
          strokeWidth={2}
          className="size-4"
        />
      </span>
    );
  }

  if (status === "cancelled") {
    return (
      <span className="flex size-7 items-center justify-center rounded-full bg-muted text-muted-foreground">
        <HugeiconsIcon
          icon={MultiplicationSignCircleIcon}
          strokeWidth={2}
          className="size-4"
        />
      </span>
    );
  }

  return (
    <span className="flex size-7 items-center justify-center rounded-full border border-border bg-background">
      <span className="size-1.5 rounded-full bg-border" />
    </span>
  );
}

function StatusMark({ kind }: { kind: ConnectionState["kind"] }) {
  const icon =
    kind === "success"
      ? CheckmarkCircle02Icon
      : kind === "error"
        ? Alert02Icon
        : kind === "cancelled"
          ? MultiplicationSignCircleIcon
          : BankIcon;

  return (
    <div
      className={cn(
        "relative flex size-14 items-center justify-center rounded-2xl border",
        kind === "success" && "border-profit/20 bg-profit/10 text-profit",
        kind === "error" && "border-loss/20 bg-loss/10 text-loss",
        kind === "cancelled" && "border-border bg-muted text-muted-foreground",
        kind === "connecting" && "border-border bg-muted text-foreground",
      )}
    >
      {kind === "connecting" ? (
        <span className="absolute inset-[-5px] animate-pulse rounded-[1.15rem] border border-foreground/10 motion-reduce:animate-none" />
      ) : null}
      <HugeiconsIcon icon={icon} strokeWidth={1.8} className="size-6" />
    </div>
  );
}

function ConnectionReceipt({
  state,
  onReturn,
}: {
  state: ConnectionState;
  onReturn?: () => void;
}) {
  const copy = COPY[state.kind];
  const steps = stepsFor(state);

  return (
    <main className="relative flex min-h-svh items-center justify-center overflow-hidden bg-[#f7f7f5] px-5 py-12 text-foreground dark:bg-[#111112]">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-0 opacity-70 [background-image:linear-gradient(to_bottom,transparent_31px,rgba(20,20,20,0.035)_32px)] [background-size:100%_32px] dark:opacity-20"
      />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute inset-y-0 left-[max(1.25rem,calc(50%-19rem))] w-px bg-foreground/[0.05]"
      />

      <a
        href="/"
        aria-label="Tradstry home"
        className="absolute left-5 top-5 inline-flex items-center gap-2 rounded-md p-1.5 text-sm font-semibold outline-none transition-opacity hover:opacity-65 focus-visible:ring-2 focus-visible:ring-ring/40 sm:left-8 sm:top-7"
      >
        <span className="flex size-7 items-center justify-center rounded-lg bg-foreground text-background">
          <TradstryMark className="size-4" />
        </span>
        Tradstry
      </a>

      <section
        aria-live="polite"
        className="relative w-full max-w-[32rem] overflow-hidden rounded-2xl border border-border/80 bg-background shadow-[0_24px_80px_-42px_rgba(0,0,0,0.45)]"
      >
        <div className="p-6 sm:p-8">
          <StatusMark kind={state.kind} />

          <p className="mt-7 font-mono text-[0.65rem] font-medium uppercase tracking-[0.16em] text-muted-foreground">
            {copy.eyebrow}
          </p>
          <h1 className="mt-2 text-balance text-2xl font-semibold tracking-[-0.025em] sm:text-[1.75rem]">
            {copy.title}
          </h1>
          <p className="mt-3 max-w-md text-sm leading-6 text-muted-foreground">
            {copy.description}
          </p>

          {state.kind === "error" ? (
            <p
              role="alert"
              className="mt-4 rounded-lg border border-loss/15 bg-loss/[0.06] px-3 py-2.5 text-xs leading-5 text-loss"
            >
              {state.detail}
            </p>
          ) : null}

          <div className="relative mt-7 border-y border-border/70">
            <div
              aria-hidden="true"
              className="absolute bottom-7 left-[0.84375rem] top-7 w-px bg-border"
            />
            {steps.map((step) => (
              <div
                key={step.label}
                className="relative grid grid-cols-[1.75rem_minmax(0,1fr)_auto] items-center gap-3 border-b border-border/60 py-3.5 last:border-b-0"
              >
                <StepMark status={step.status} />
                <span
                  className={cn(
                    "text-sm",
                    step.status === "pending" && "text-muted-foreground",
                  )}
                >
                  {step.label}
                </span>
                <span
                  className={cn(
                    "font-mono text-[0.65rem] uppercase tracking-wide text-muted-foreground",
                    step.status === "active" && "text-foreground",
                    step.status === "error" && "text-loss",
                    step.status === "cancelled" && "text-muted-foreground",
                    step.status === "complete" && "text-profit",
                  )}
                >
                  {step.detail}
                </span>
              </div>
            ))}
          </div>

          <div className="mt-6 flex min-h-8 items-center justify-between gap-4">
            <p className="text-xs leading-5 text-muted-foreground">
              {state.kind === "success"
                ? "Returning to Brokerage automatically…"
                : state.kind === "connecting"
                  ? "This usually takes only a few seconds."
                  : "Your brokerage sign-in remains with the provider."}
            </p>
            {state.kind !== "connecting" && onReturn ? (
              <Button size="lg" onClick={onReturn} className="shrink-0">
                {state.kind === "success"
                  ? "Open brokerage"
                  : "Back to brokerage"}
                <HugeiconsIcon
                  icon={ArrowRight01Icon}
                  strokeWidth={2}
                  data-icon="inline-end"
                />
              </Button>
            ) : null}
          </div>
        </div>

        <div
          className={cn(
            "h-1 w-full bg-muted",
            state.kind === "success" && "bg-profit/15",
            state.kind === "error" && "bg-loss/15",
          )}
        >
          {state.kind === "connecting" ? (
            <div className="h-full w-1/3 animate-pulse bg-foreground/60 motion-reduce:animate-none" />
          ) : null}
          {state.kind === "success" ? (
            <div className="h-full w-full bg-profit" />
          ) : null}
          {state.kind === "error" ? (
            <div className="h-full w-1/3 bg-loss" />
          ) : null}
          {state.kind === "cancelled" ? (
            <div className="h-full w-1/3 bg-muted-foreground/35" />
          ) : null}
        </div>
      </section>
    </main>
  );
}

function CallbackHandler() {
  const searchParams = useSearchParams();
  const router = useRouter();
  const fetcher = useGraphQL();
  const didRun = useRef(false);
  const [state, setState] = useState<ConnectionState>({ kind: "connecting" });

  useEffect(() => {
    if (didRun.current) return;
    didRun.current = true;

    const status = searchParams.get("status");
    const connectionId = searchParams.get("connection_id");
    const workspaceId = searchParams.get("workspaceId");

    async function completeConnection() {
      if (status === "SUCCESS" && connectionId && workspaceId) {
        try {
          await brokerageService.completeConnection(
            fetcher,
            workspaceId,
            connectionId,
          );
          setState({ kind: "success" });
          window.setTimeout(() => {
            router.replace("/dashboard/brokerage");
          }, 2200);
        } catch {
          setState({
            kind: "error",
            detail:
              "The connection could not be verified. Return to Brokerage and reconnect the account.",
          });
        }
        return;
      }

      if (status === "ERROR") {
        const errorCode = searchParams.get("error_code");
        setState({
          kind: "error",
          detail: errorCode
            ? `The brokerage provider returned: ${errorCode.toLowerCase().replaceAll("_", " ")}.`
            : "The brokerage provider did not complete the authorization.",
        });
        return;
      }

      if (status === "ABANDONED") {
        setState({ kind: "cancelled" });
        return;
      }

      setState({
        kind: "error",
        detail:
          "This callback is missing connection details. Return to Brokerage and start the connection again.",
      });
    }

    completeConnection();
  }, [searchParams, router, fetcher]);

  return (
    <ConnectionReceipt
      state={state}
      onReturn={() => router.replace("/dashboard/brokerage")}
    />
  );
}

export default function BrokerageCallbackPage() {
  return (
    <GraphQLProvider>
      <Suspense fallback={<ConnectionReceipt state={{ kind: "connecting" }} />}>
        <CallbackHandler />
      </Suspense>
    </GraphQLProvider>
  );
}
