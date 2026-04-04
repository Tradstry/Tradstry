"use client";

import { useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Loading01Icon, CheckmarkCircle01Icon } from "@hugeicons/core-free-icons";
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible";
import type { ThinkingStep } from "@/hooks/chat";

interface ChatStreamMessageProps {
  content: string;
  thinkingSteps: ThinkingStep[];
}

const TOOL_LABELS: Record<string, string> = {
  db_query: "Database Query",
  semantic_search: "Semantic Search",
  analytics_calc: "Analytics",
  create_agent: "Create Agent",
  edit_agent: "Edit Agent",
  run_agent: "Run Agent",
  save_agent: "Save Agent",
};

function formatToolName(name: string): string {
  return TOOL_LABELS[name] ?? name.replace(/_/g, " ");
}

function truncate(text: string, max: number): string {
  if (text.length <= max) return text;
  return text.slice(0, max) + "...";
}

function formatArgs(argsJson: string): string {
  try {
    const parsed = JSON.parse(argsJson);
    const parts: string[] = [];
    for (const [key, value] of Object.entries(parsed)) {
      if (value === null || value === undefined || value === "") continue;
      const display = typeof value === "string" ? value : JSON.stringify(value);
      parts.push(`${key}: ${display}`);
    }
    return parts.length > 0 ? truncate(parts.join(", "), 120) : "";
  } catch {
    return truncate(argsJson, 120);
  }
}

function formatResult(result: string): string {
  return truncate(result.replace(/\n/g, " "), 150);
}

function cleanContent(text: string): string {
  return text
    .replace(/\*\*/g, "")
    .replace(/\*/g, "")
    .replace(/[—–]/g, "-")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\|[-\s|:]+\|$/gm, "")
    .replace(/\|/g, "  ")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

export function ThinkingCollapsible({
  steps,
  defaultOpen = true,
}: {
  steps: ThinkingStep[];
  defaultOpen?: boolean;
}) {
  const [open, setOpen] = useState(defaultOpen);
  const isThinking = steps.some((s) => s.status === "running");
  const doneCount = steps.filter((s) => s.status === "done").length;

  return (
    <Collapsible open={open} onOpenChange={setOpen}>
      <CollapsibleTrigger className="flex w-full items-center gap-1.5 rounded-lg bg-muted px-3 py-2 text-xs text-muted-foreground transition-colors hover:bg-muted/80">
        {isThinking ? (
          <HugeiconsIcon icon={Loading01Icon} className="size-3 animate-spin shrink-0" />
        ) : (
          <HugeiconsIcon icon={CheckmarkCircle01Icon} className="size-3 shrink-0 text-emerald-500" />
        )}
        <span className="font-medium">
          {isThinking ? "Thinking" : "Thought"}
        </span>
        <span className="text-muted-foreground/60">
          ({doneCount}/{steps.length} steps)
        </span>
        <svg
          width="12"
          height="12"
          viewBox="0 0 16 16"
          fill="none"
          className={`ml-auto shrink-0 transition-transform ${open ? "rotate-180" : ""}`}
        >
          <path d="M4 6l4 4 4-4" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round" />
        </svg>
      </CollapsibleTrigger>
      <CollapsibleContent>
        <div className="mt-1 space-y-1">
          {steps.map((step, i) => (
            <div
              key={`${step.toolName}-${i}`}
              className="rounded-md border border-border/50 bg-muted/50 px-2.5 py-1.5"
            >
              <div className="flex items-center gap-1.5">
                {step.status === "running" ? (
                  <HugeiconsIcon icon={Loading01Icon} className="size-2.5 animate-spin shrink-0 text-muted-foreground" />
                ) : (
                  <HugeiconsIcon icon={CheckmarkCircle01Icon} className="size-2.5 shrink-0 text-emerald-500" />
                )}
                <span className="text-[0.7rem] font-medium text-foreground">
                  {formatToolName(step.toolName)}
                </span>
              </div>
              {step.args && (
                <p className="mt-0.5 text-[0.65rem] text-muted-foreground">
                  {formatArgs(step.args)}
                </p>
              )}
              {step.result && (
                <p className="mt-0.5 text-[0.65rem] text-muted-foreground/70 italic">
                  {formatResult(step.result)}
                </p>
              )}
            </div>
          ))}
        </div>
      </CollapsibleContent>
    </Collapsible>
  );
}

export function ChatStreamMessage({ content, thinkingSteps }: ChatStreamMessageProps) {
  const hasSteps = thinkingSteps.length > 0;

  return (
    <div className="flex justify-start">
      <div className="max-w-[80%] space-y-2">
        {hasSteps && <ThinkingCollapsible steps={thinkingSteps} defaultOpen={true} />}

        {/* Response content or initial thinking indicator */}
        {content ? (
          <div className="whitespace-pre-wrap rounded-lg bg-muted px-3 py-2 text-xs/relaxed text-foreground">
            {cleanContent(content)}
            <span className="ml-0.5 inline-block animate-pulse">&#9612;</span>
          </div>
        ) : !hasSteps ? (
          <div className="rounded-lg bg-muted px-3 py-2 text-xs/relaxed">
            <span className="flex items-center gap-1.5 text-muted-foreground">
              <HugeiconsIcon icon={Loading01Icon} className="size-3 animate-spin" />
              Thinking...
            </span>
          </div>
        ) : null}
      </div>
    </div>
  );
}
