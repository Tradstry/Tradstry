"use client";

import { useEffect, useRef } from "react";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useChatMessages, useChatStore } from "@/hooks/chat";
import { ChatStreamMessage } from "./chat-stream-message";

interface ChatMessageListProps {
  sessionId: string;
}

export function ChatMessageList({ sessionId }: ChatMessageListProps) {
  const { data: messages = [] } = useChatMessages(sessionId);
  const { isStreaming, streamingMessage, streamingToolName, optimisticUserMessage } = useChatStore();
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages, streamingMessage, isStreaming, optimisticUserMessage]);

  const visibleMessages = messages.filter((m) => m.role !== "tool");

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
        {visibleMessages.length === 0 && !isStreaming && (
          <div className="flex h-32 items-center justify-center text-xs text-muted-foreground">
            No messages yet. Start a conversation!
          </div>
        )}

        {visibleMessages.map((message) => (
          <div
            key={message.id}
            className={`flex ${message.role === "user" ? "justify-end" : "justify-start"}`}
          >
            <div
              className={`max-w-[80%] whitespace-pre-wrap rounded-lg px-3 py-2 text-xs/relaxed ${
                message.role === "user"
                  ? "bg-primary text-primary-foreground"
                  : "bg-muted text-foreground"
              }`}
            >
              {message.role === "user" ? message.content : renderContent(message.content)}
            </div>
          </div>
        ))}

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
            toolName={streamingToolName}
          />
        )}

        <div ref={bottomRef} />
      </div>
    </ScrollArea>
  );
}
