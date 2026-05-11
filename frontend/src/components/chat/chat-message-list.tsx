"use client";

import { useEffect, useRef } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useChatMessages, useChatStore, useSendMessage } from "@/hooks/chat";
import type { ThinkingStep } from "@/hooks/chat";
import type { ChatMessage } from "@/lib/types/chat";
import { useActiveAccount } from "@/components/accounts";
import { ChatStreamMessage } from "./chat-stream-message";
import { ThinkingCollapsible } from "./chat-stream-message";

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
      groups.push({ kind: "assistant", toolMessages: pendingTools, message: msg });
      pendingTools = [];
    }
  }
  return groups;
}

function toolMessagesToSteps(toolMessages: ChatMessage[]): ThinkingStep[] {
  return toolMessages.map((msg) => ({
    toolName: msg.toolName ?? "unknown",
    args: null,
    result: msg.content || null,
    status: "done" as const,
  }));
}

export function ChatMessageList({ sessionId }: ChatMessageListProps) {
  const { data: messages = [] } = useChatMessages(sessionId);
  const { isStreaming, streamingMessage, reasoningText, thinkingSteps, optimisticUserMessage, streamError, lastFailedMessage, clearError } = useChatStore();
  const account = useActiveAccount();
  const sendMessage = useSendMessage(account?.id ?? null);
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingMessage, isStreaming, optimisticUserMessage]);

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

  return (
    <ScrollArea className="h-full">
      <div className="flex flex-col gap-3 px-3 py-3">
        {groups.length === 0 && !isStreaming && (
          <div className="flex h-32 items-center justify-center text-xs text-muted-foreground">
            No messages yet. Start a conversation!
          </div>
        )}

        {groups.map((group) => {
          if (group.kind === "user") {
            return (
              <div key={group.message.id} className="flex justify-end">
                <div className="max-w-[80%] whitespace-pre-wrap rounded-lg bg-primary px-3 py-2 text-xs/relaxed text-primary-foreground">
                  {group.message.content}
                </div>
              </div>
            );
          }

          const steps = toolMessagesToSteps(group.toolMessages);
          return (
            <div key={group.message.id} className="flex justify-start">
              <div className="max-w-[80%] space-y-2">
                {steps.length > 0 && (
                  <ThinkingCollapsible steps={steps} defaultOpen={false} />
                )}
                <div className="whitespace-pre-wrap rounded-lg bg-muted px-3 py-2 text-xs/relaxed text-foreground">
                  {renderContent(group.message.content)}
                </div>
              </div>
            </div>
          );
        })}

        {optimisticUserMessage && (
          <div className="flex justify-end">
            <div className="max-w-[80%] rounded-lg bg-primary px-3 py-2 text-xs/relaxed text-primary-foreground">
              {optimisticUserMessage}
            </div>
          </div>
        )}

        {(isStreaming || optimisticUserMessage) && (
          <ChatStreamMessage
            content={streamingMessage}
            reasoningText={reasoningText}
            thinkingSteps={thinkingSteps}
            isStreaming={isStreaming}
          />
        )}

        {streamError && (
          <div className="flex justify-start">
            <div className="max-w-[80%] rounded-lg border border-destructive/30 bg-destructive/10 px-3 py-2 text-xs/relaxed text-destructive">
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

        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
