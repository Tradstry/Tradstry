"use client";

import { useEffect, useRef, useState } from "react";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import { useChatMessages, useChatStore, useSendMessage } from "@tradstry/app-ui/hooks/chat";
import { useJournalEntriesForWorkspace } from "@tradstry/app-ui/hooks/journal";
import { usePlaybooks } from "@tradstry/app-ui/hooks/playbook";
import {
  type ChatContext,
  type ChatMessage,
  redactInternalIds,
} from "@tradstry/app-ui/lib/types/chat";
import type { JournalEntry } from "@tradstry/app-ui/lib/types/journal";
import type { PlaybookWithStats } from "@tradstry/app-ui/lib/types/playbook";
import { ChatStreamMessage } from "./chat-stream-message";

interface ChatMessageListProps {
  sessionId: string;
}

// Group messages into render units: user messages standalone,
// consecutive tool messages + following assistant message grouped together.
type MessageGroup =
  | { kind: "user"; message: ChatMessage }
  | { kind: "assistant"; toolMessages: ChatMessage[]; message: ChatMessage };

function groupMessages(messages: ChatMessage[]): MessageGroup[] {
  const groups: MessageGroup[] = [];
  let pendingTools: ChatMessage[] = [];

  for (const msg of messages) {
    if (msg.role === "user") {
      // Flush any orphaned tool messages (shouldn't happen, but be safe)
      pendingTools = [];
      groups.push({ kind: "user", message: msg });
    } else if (msg.role === "tool") {
      pendingTools.push(msg);
    } else if (msg.role === "assistant") {
      groups.push({
        kind: "assistant",
        toolMessages: pendingTools,
        message: msg,
      });
      pendingTools = [];
    }
  }
  return groups;
}

function asStringArray(value: unknown): string[] | undefined {
  if (!Array.isArray(value)) return undefined;
  const strings = value.filter(
    (item): item is string => typeof item === "string",
  );
  return strings.length > 0 ? strings : undefined;
}

function parseMessageContext(contextJson: string | null): ChatContext | null {
  if (!contextJson) return null;

  try {
    const value: unknown = JSON.parse(contextJson);
    if (!value || typeof value !== "object" || Array.isArray(value))
      return null;

    const record = value as Record<string, unknown>;
    const rawDateRange = record.dateRange ?? record.date_range;
    const dateRange =
      rawDateRange &&
      typeof rawDateRange === "object" &&
      !Array.isArray(rawDateRange)
        ? (rawDateRange as Record<string, unknown>)
        : null;
    const from = dateRange?.from;
    const to = dateRange?.to;
    const context: ChatContext = {
      tradeIds: asStringArray(record.tradeIds ?? record.trade_ids),
      playbookIds: asStringArray(record.playbookIds ?? record.playbook_ids),
      dateRange:
        typeof from === "string" && typeof to === "string"
          ? { from, to }
          : undefined,
    };

    return context.tradeIds?.length ||
      context.playbookIds?.length ||
      context.dateRange
      ? context
      : null;
  } catch {
    return null;
  }
}

function shortContextDate(value: string): string {
  const date = new Date(`${value}T00:00:00`);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleDateString(undefined, { month: "short", day: "numeric" });
}

function MessageContextChips({
  context,
  trades,
  playbooks,
}: {
  context: ChatContext | null | undefined;
  trades: JournalEntry[];
  playbooks: PlaybookWithStats[];
}) {
  if (!context) return null;

  return (
    <div className="flex max-w-full flex-wrap gap-1.5">
      {context.tradeIds?.map((tradeId) => {
        const trade = trades.find((item) => item.id === tradeId);
        return (
          <span
            key={tradeId}
            className="inline-flex max-w-full items-center gap-1 rounded-full border border-primary-foreground/15 bg-primary-foreground/10 px-2 py-1 text-[0.65rem]/none font-medium text-primary-foreground/85"
            title={
              trade
                ? `${trade.symbol} ${trade.tradeType} trade`
                : "Tagged trade"
            }
          >
            <span className="text-primary-foreground/55">@</span>
            <span className="truncate">{trade?.symbol ?? "Trade"}</span>
            {trade ? (
              <span className="text-[0.55rem] uppercase tracking-wide text-primary-foreground/50">
                {trade.tradeType}
              </span>
            ) : null}
          </span>
        );
      })}
      {context.playbookIds?.map((playbookId) => {
        const playbook = playbooks.find((item) => item.id === playbookId);
        return (
          <span
            key={playbookId}
            className="inline-flex max-w-full items-center gap-1 rounded-full border border-primary-foreground/15 bg-primary-foreground/10 px-2 py-1 text-[0.65rem]/none font-medium text-primary-foreground/85"
            title={
              playbook ? `${playbook.name} playbook` : `Playbook ${playbookId}`
            }
          >
            <span className="text-primary-foreground/55">@</span>
            <span className="truncate">{playbook?.name ?? "Playbook"}</span>
          </span>
        );
      })}
      {context.dateRange ? (
        <span className="inline-flex items-center rounded-full border border-primary-foreground/15 bg-primary-foreground/10 px-2 py-1 text-[0.65rem]/none font-medium text-primary-foreground/85">
          {shortContextDate(context.dateRange.from)}–
          {shortContextDate(context.dateRange.to)}
        </span>
      ) : null}
    </div>
  );
}

function UserMessageBubble({
  message,
  context,
  trades,
  playbooks,
}: {
  message: string;
  context: ChatContext | null | undefined;
  trades: JournalEntry[];
  playbooks: PlaybookWithStats[];
}) {
  return (
    <div className="flex max-w-[80%] flex-col items-start gap-2 rounded-lg bg-primary px-4 py-3 text-primary-foreground">
      <MessageContextChips
        context={context}
        trades={trades}
        playbooks={playbooks}
      />
      <p className="whitespace-pre-wrap text-xs/relaxed">{message}</p>
    </div>
  );
}

export function ChatMessageList({ sessionId }: ChatMessageListProps) {
  const { data: messages = [] } = useChatMessages(sessionId);
  const {
    isStreaming,
    streamingMessage,
    optimisticUserMessage,
    streamError,
    lastFailedMessage,
    clearError,
  } = useChatStore();
  const account = useActiveWorkspace();
  const { data: trades = [] } = useJournalEntriesForWorkspace(
    account?.id ?? null,
  );
  const { data: playbooks = [] } = usePlaybooks();
  const sendMessage = useSendMessage(account?.id ?? null);
  const contentRef = useRef<HTMLDivElement>(null);
  const turnsRef = useRef<HTMLDivElement>(null);
  const anchorRef = useRef<HTMLDivElement>(null);
  const prevUserCount = useRef(0);
  const [spacerHeight, setSpacerHeight] = useState(0);

  const userTurnCount =
    messages.filter((m) => m.role === "user").length +
    (optimisticUserMessage ? 1 : 0);

  // Reset the turn baseline when switching sessions so the first load re-anchors.
  useEffect(() => {
    if (!sessionId) return;
    prevUserCount.current = 0;
  }, [sessionId]);

  // Size a bottom spacer so the latest question can always scroll to the top of
  // the viewport, then pin it there whenever a new turn begins. The answer
  // streams in below it; the user stays anchored until they scroll up themselves.
  // biome-ignore lint/correctness/useExhaustiveDependencies: message and stream changes resize the measured conversation even when the turn count is unchanged.
  useEffect(() => {
    const content = contentRef.current;
    const turns = turnsRef.current;
    const anchor = anchorRef.current;
    const viewport = content?.closest<HTMLElement>(
      '[data-slot="scroll-area-viewport"]',
    );
    if (!content || !turns || !viewport) return;

    if (anchor) {
      const below =
        turns.getBoundingClientRect().bottom -
        anchor.getBoundingClientRect().top;
      setSpacerHeight(Math.max(0, viewport.clientHeight - below));
    } else {
      setSpacerHeight(0);
    }

    if (userTurnCount > prevUserCount.current) {
      prevUserCount.current = userTurnCount;
      // Defer so the spacer height is applied before we scroll.
      requestAnimationFrame(() => {
        anchor?.scrollIntoView({ block: "start", behavior: "smooth" });
      });
    }
  }, [
    messages,
    streamingMessage,
    isStreaming,
    optimisticUserMessage,
    userTurnCount,
  ]);

  function handleRetry() {
    if (!lastFailedMessage) return;
    clearError();
    sendMessage.mutate({
      sessionId: lastFailedMessage.sessionId,
      content: lastFailedMessage.content,
      context: lastFailedMessage.context,
    });
  }

  const groups = groupMessages(messages);

  function cleanContent(text: string): string {
    return redactInternalIds(text)
      .replace(/\*\*/g, "")
      .replace(/\*/g, "")
      .replace(/[—–]/g, "-")
      .replace(/^#{1,6}\s+/gm, "")
      .replace(/^\|[-\s|:]+\|$/gm, "")
      .replace(/\|/g, "  ")
      .replace(/\n{3,}/g, "\n\n")
      .trim();
  }

  function renderContent(text: string) {
    return cleanContent(text);
  }

  // The most recent user turn is the scroll anchor. When the optimistic message
  // is showing it's the freshest question; otherwise it's the last user group.
  let lastUserGroupIdx = -1;
  groups.forEach((g, i) => {
    if (g.kind === "user") lastUserGroupIdx = i;
  });

  return (
    <ScrollArea className="h-full">
      <div ref={contentRef} className="px-3 pt-4 pb-3">
        <div ref={turnsRef} className="flex flex-col gap-3">
          {groups.length === 0 && !isStreaming && (
            <div className="flex h-32 items-center justify-center text-xs text-muted-foreground">
              No messages yet. Start a conversation!
            </div>
          )}

          {groups.map((group, i) => {
            if (group.kind === "user") {
              const isAnchor = i === lastUserGroupIdx && !optimisticUserMessage;
              return (
                <div
                  key={group.message.id}
                  ref={isAnchor ? anchorRef : undefined}
                  className="flex scroll-mt-4 justify-end"
                >
                  <UserMessageBubble
                    message={group.message.content}
                    context={parseMessageContext(group.message.contextJson)}
                    trades={trades}
                    playbooks={playbooks}
                  />
                </div>
              );
            }

            if (group.message.content.trim() === "") return null;
            return (
              <div key={group.message.id} className="flex justify-start">
                <div className="max-w-[80%]">
                  <div className="whitespace-pre-wrap rounded-lg bg-muted px-4 py-3 text-xs/relaxed text-foreground">
                    {renderContent(group.message.content)}
                  </div>
                </div>
              </div>
            );
          })}

          {optimisticUserMessage && (
            <div ref={anchorRef} className="flex scroll-mt-4 justify-end">
              <UserMessageBubble
                message={optimisticUserMessage}
                context={lastFailedMessage?.context}
                trades={trades}
                playbooks={playbooks}
              />
            </div>
          )}

          {(isStreaming || optimisticUserMessage) && (
            <ChatStreamMessage
              content={streamingMessage}
              isStreaming={isStreaming}
            />
          )}

          {streamError && (
            <div className="flex justify-start">
              <div className="max-w-[80%] rounded-lg border border-destructive/30 bg-destructive/10 px-4 py-3 text-xs/relaxed text-destructive">
                <p>{streamError}</p>
                <div className="mt-2 flex gap-2">
                  {lastFailedMessage && (
                    <button
                      type="button"
                      onClick={handleRetry}
                      className="rounded-md bg-destructive/15 px-2.5 py-1 text-xs font-medium text-destructive hover:bg-destructive/25 transition-colors"
                    >
                      Try again
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={clearError}
                    className="rounded-md px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
                  >
                    Dismiss
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>

        <div aria-hidden style={{ height: spacerHeight }} />
      </div>
    </ScrollArea>
  );
}
