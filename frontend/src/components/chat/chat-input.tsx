"use client";

import { useState, useRef, useEffect, KeyboardEvent, ChangeEvent } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { useChatStore, useSendMessage } from "@/hooks/chat";
import { ChatContextPicker } from "./chat-context-picker";

interface ChatInputProps {
  sessionId: string;
  accountId: string;
}

const SLASH_COMMANDS = [
  {
    command: "/report",
    label: "Generate Report",
    description: "Full performance report for a date range",
    prompt:
      "Generate a full trading performance report including total P&L, win rate, average R, profit factor, streaks, and per-symbol breakdown.",
  },
  {
    command: "/analysis",
    label: "Generate Analysis",
    description: "Deep analysis of patterns and insights",
    prompt:
      "Analyze my recent trading patterns. Look for recurring setups, common mistakes, and actionable insights to improve my performance.",
  },
];

export function ChatInput({ sessionId, accountId }: ChatInputProps) {
  const [text, setText] = useState("");
  const [pickerOpen, setPickerOpen] = useState(false);
  const [slashOpen, setSlashOpen] = useState(false);
  const [slashIndex, setSlashIndex] = useState(0);
  const { isStreaming, pinnedContext, clearPinnedContext, resetStream } =
    useChatStore();
  const sendMessage = useSendMessage(accountId);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const hasPinnedContext =
    !!pinnedContext.dateRange ||
    (pinnedContext.tradeIds && pinnedContext.tradeIds.length > 0) ||
    (pinnedContext.playbookIds && pinnedContext.playbookIds.length > 0);

  // Filter commands based on what user typed after /
  const query = slashOpen ? text.slice(1).toLowerCase() : "";
  const filtered = SLASH_COMMANDS.filter(
    (cmd) =>
      cmd.command.toLowerCase().includes(query) ||
      cmd.label.toLowerCase().includes(query)
  );

  // Close slash menu if no matches or text no longer starts with /
  useEffect(() => {
    if (slashOpen && (!text.startsWith("/") || filtered.length === 0)) {
      setSlashOpen(false);
    }
  }, [text, slashOpen, filtered.length]);

  // Clamp index when filtered list changes
  useEffect(() => {
    if (slashIndex >= filtered.length) {
      setSlashIndex(Math.max(0, filtered.length - 1));
    }
  }, [filtered.length, slashIndex]);

  function handleSend() {
    const content = text.trim();
    if (!content || isStreaming) return;
    resetStream();
    sendMessage.mutate({
      sessionId,
      content,
      context: hasPinnedContext ? pinnedContext : undefined,
    });
    setText("");
    textareaRef.current?.focus();
  }

  function selectSlashCommand(cmd: (typeof SLASH_COMMANDS)[number]) {
    setSlashOpen(false);
    setText("");
    resetStream();
    sendMessage.mutate({
      sessionId,
      content: cmd.prompt,
      context: hasPinnedContext ? pinnedContext : undefined,
    });
    textareaRef.current?.focus();
  }

  function handleChange(e: ChangeEvent<HTMLTextAreaElement>) {
    const val = e.target.value;
    setText(val);

    // Open slash menu when user types "/" at start of empty input
    if (val === "/") {
      setSlashOpen(true);
      setSlashIndex(0);
    }
  }

  function handleKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (slashOpen && filtered.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        setSlashIndex((i) => (i + 1) % filtered.length);
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        setSlashIndex((i) => (i - 1 + filtered.length) % filtered.length);
        return;
      }
      if (e.key === "Enter") {
        e.preventDefault();
        selectSlashCommand(filtered[slashIndex]);
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        setSlashOpen(false);
        return;
      }
    }

    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  }

  return (
    <div className="relative px-3 py-3">
      <div className="rounded-2xl border border-border bg-background p-3 shadow-sm">
        {/* Pinned context chips */}
        {hasPinnedContext && (
          <div className="mb-2 flex flex-wrap gap-1">
            {pinnedContext.dateRange && (
              <span className="inline-flex items-center gap-1 rounded-full border border-border bg-muted px-2 py-0.5 text-xs text-foreground">
                {pinnedContext.dateRange.from} – {pinnedContext.dateRange.to}
                <button
                  onClick={clearPinnedContext}
                  className="ml-0.5 text-muted-foreground hover:text-foreground"
                  aria-label="Remove date range"
                >
                  <HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
                </button>
              </span>
            )}
          </div>
        )}

        {/* @ Add context button */}
        <div className="mb-2">
          <button
            onClick={() => setPickerOpen((v) => !v)}
            disabled={isStreaming}
            className="inline-flex items-center gap-1.5 rounded-full border border-border px-3 py-1 text-xs text-muted-foreground transition-colors hover:bg-muted hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
          >
            <span className="font-medium">@</span>
            Add context
          </button>
          {pickerOpen && (
            <ChatContextPicker onClose={() => setPickerOpen(false)} />
          )}
        </div>

        {/* Slash command popover */}
        {slashOpen && filtered.length > 0 && (
          <div className="absolute bottom-full left-3 right-3 z-50 mb-2 overflow-hidden rounded-xl border border-border bg-popover shadow-md">
            {filtered.map((cmd, i) => (
              <button
                key={cmd.command}
                onMouseDown={(e) => {
                  e.preventDefault();
                  selectSlashCommand(cmd);
                }}
                className={`flex w-full flex-col gap-0.5 px-3 py-2.5 text-left transition-colors ${
                  i === slashIndex ? "bg-muted" : "hover:bg-muted/50"
                }`}
              >
                <span className="text-sm font-medium text-foreground">
                  {cmd.label}
                </span>
                <span className="text-xs text-muted-foreground">
                  {cmd.description}
                </span>
              </button>
            ))}
          </div>
        )}

        {/* Textarea */}
        <textarea
          ref={textareaRef}
          value={text}
          onChange={handleChange}
          onKeyDown={handleKeyDown}
          disabled={isStreaming}
          placeholder="Ask, search, or make anything..."
          rows={1}
          className="max-h-32 min-h-[1.5rem] w-full resize-none bg-transparent text-sm text-foreground placeholder:text-muted-foreground focus:outline-none disabled:cursor-not-allowed disabled:opacity-50"
          style={{ overflowY: "auto" }}
        />

        {/* Bottom toolbar */}
        <div className="flex items-center justify-between pt-2">
          <div />

          {/* Send button */}
          <button
            onClick={handleSend}
            disabled={isStreaming || !text.trim()}
            className="flex size-8 items-center justify-center rounded-full bg-foreground text-background transition-opacity hover:opacity-80 disabled:cursor-not-allowed disabled:opacity-30"
            title="Send"
          >
            <svg width="16" height="16" viewBox="0 0 16 16" fill="none">
              <path
                d="M8 13V3M8 3l4 4M8 3L4 7"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            </svg>
          </button>
        </div>
      </div>
    </div>
  );
}
