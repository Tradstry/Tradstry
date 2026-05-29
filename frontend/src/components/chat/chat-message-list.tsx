"use client";

import { useEffect, useRef, useState } from "react";
import { useActiveAccount } from "@/components/accounts";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useChatMessages, useChatStore, useSendMessage } from "@/hooks/chat";
import type { ChatMessage } from "@/lib/types/chat";
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
  const account = useActiveAccount();
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
    prevUserCount.current = 0;
  }, [sessionId]);

  // Size a bottom spacer so the latest question can always scroll to the top of
  // the viewport, then pin it there whenever a new turn begins. The answer
  // streams in below it; the user stays anchored until they scroll up themselves.
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
  }, [messages, streamingMessage, isStreaming, optimisticUserMessage, userTurnCount]);

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

  function renderContent(text: string) {
    const cleaned = cleanContent(text);
    return cleaned.split("\n").map((line, i) => {
      if (line.trim() === "") return <br key={i} />;
      return (
        <span key={i}>
          {line}
          {"\n"}
        </span>
      );
    });
  }

  // The most recent user turn is the scroll anchor. When the optimistic message
  // is showing it's the freshest question; otherwise it's the last user group.
  let lastUserGroupIdx = -1;
  groups.forEach((g, i) => {
    if (g.kind === "user") lastUserGroupIdx = i;
  });

  return (
    <ScrollArea className="h-full">
      <div ref={contentRef} className="px-3 py-3">
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
                  className="flex justify-end"
                >
                  <div className="max-w-[80%] whitespace-pre-wrap rounded-lg bg-primary px-4 py-3 text-xs/relaxed text-primary-foreground">
                    {group.message.content}
                  </div>
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
            <div ref={anchorRef} className="flex justify-end">
              <div className="max-w-[80%] rounded-lg bg-primary px-4 py-3 text-xs/relaxed text-primary-foreground">
                {optimisticUserMessage}
              </div>
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
                      onClick={handleRetry}
                      className="rounded-md bg-destructive/15 px-2.5 py-1 text-xs font-medium text-destructive hover:bg-destructive/25 transition-colors"
                    >
                      Try again
                    </button>
                  )}
                  <button
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
