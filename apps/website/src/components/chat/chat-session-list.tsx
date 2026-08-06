"use client";

import { useState } from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Add01Icon, Delete02Icon } from "@hugeicons/core-free-icons";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import {
  useChatSessions,
  useCreateSession,
  useDeleteSession,
  useSendMessage,
  useChatStore,
} from "@/hooks/chat";

interface ChatSessionListProps {
  workspaceId: string;
}

const STARTER_PROMPTS = [
  {
    label: "Analyze my recent trades",
    prompt:
      "Analyze my recent trading patterns. Look for recurring setups, common mistakes, and actionable insights to improve my performance.",
  },
  {
    label: "Generate a performance report",
    prompt:
      "Generate a full trading performance report including total P&L, win rate, average R, profit factor, streaks, and per-symbol breakdown.",
  },
  {
    label: "What's my win rate and edge?",
    prompt:
      "What's my win rate, average R-multiple, and overall edge? Summarize my key performance metrics and what's driving them.",
  },
];

function timeAgo(dateStr: string): string {
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const seconds = Math.floor((now - then) / 1000);

  if (seconds < 60) return "just now";
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  return `${months}mo ago`;
}

export function ChatSessionList({ workspaceId }: ChatSessionListProps) {
  const { data: sessions = [] } = useChatSessions(workspaceId);
  const createSession = useCreateSession(workspaceId);
  const deleteSession = useDeleteSession(workspaceId);
  const sendMessage = useSendMessage(workspaceId);
  const { setActiveSession, resetStream } = useChatStore();
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  const isStarting = createSession.isPending;

  // Create a fresh session, then fire the starter prompt into it. The create
  // mutation's own onSuccess switches the view to the new session, so the
  // streamed answer lands in the right place.
  function startWithPrompt(content: string) {
    createSession.mutate(undefined, {
      onSuccess: (session) => {
        resetStream();
        sendMessage.mutate({ sessionId: session.id, content });
      },
    });
  }

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* New Chat button */}
      <div className="shrink-0 px-4 py-3">
        <Button
          className="w-full justify-start gap-2 rounded-lg text-sm"
          onClick={() => createSession.mutate()}
          disabled={isStarting}
        >
          <HugeiconsIcon icon={Add01Icon} className="size-4" />
          New Chat
        </Button>
      </div>

      {/* Session list or empty state */}
      <div className="flex-1 overflow-y-auto px-4">
        {sessions.length === 0 ? (
          <div className="flex flex-col gap-4 pt-6">
            <div className="text-center">
              <p className="text-sm font-medium text-foreground">
                Ask anything about your trading
              </p>
              <p className="mt-1 text-xs text-muted-foreground">
                Start with a suggestion or open a new chat.
              </p>
            </div>
            <div className="flex flex-col gap-2">
              {STARTER_PROMPTS.map((starter) => (
                <button
                  key={starter.label}
                  type="button"
                  onClick={() => startWithPrompt(starter.prompt)}
                  disabled={isStarting}
                  className="rounded-lg border border-border bg-card px-3 py-2.5 text-left text-xs text-foreground transition-colors hover:bg-muted disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {starter.label}
                </button>
              ))}
            </div>
          </div>
        ) : (
          sessions.map((session) => (
            <div
              key={session.id}
              className="group relative flex items-center rounded-lg transition-colors hover:bg-muted"
            >
              <button
                type="button"
                onClick={() => setActiveSession(session.id)}
                className="w-full px-2 py-3 text-left"
              >
                <p className="truncate pr-8 text-sm font-medium text-foreground">
                  {session.title ?? `Chat ${session.id.slice(0, 8)}`}
                </p>
                <div className="mt-1 flex items-center gap-1 text-xs text-muted-foreground">
                  <svg
                    width="12"
                    height="12"
                    viewBox="0 0 24 24"
                    fill="none"
                    stroke="currentColor"
                    strokeWidth="2"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  >
                    <circle cx="12" cy="12" r="10" />
                    <polyline points="12 6 12 12 16 14" />
                  </svg>
                  {timeAgo(session.updatedAt || session.createdAt)}
                </div>
              </button>

              {/* Delete button — visible on hover */}
              <Popover
                open={confirmDeleteId === session.id}
                onOpenChange={(open) =>
                  setConfirmDeleteId(open ? session.id : null)
                }
              >
                <PopoverTrigger asChild>
                  <button
                    type="button"
                    onClick={(e) => {
                      e.stopPropagation();
                      setConfirmDeleteId(session.id);
                    }}
                    className="absolute right-2 top-1/2 -translate-y-1/2 rounded-md p-1 text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/10 hover:text-destructive group-hover:opacity-100"
                    aria-label="Delete chat"
                  >
                    <HugeiconsIcon icon={Delete02Icon} className="size-3.5" />
                  </button>
                </PopoverTrigger>
                <PopoverContent side="bottom" align="end" className="w-56 p-3">
                  <p className="text-sm font-medium">Delete this chat?</p>
                  <p className="mt-1 text-xs text-muted-foreground">
                    This action cannot be undone.
                  </p>
                  <div className="mt-3 flex justify-end gap-2">
                    <Button
                      variant="ghost"
                      size="sm"
                      onClick={() => setConfirmDeleteId(null)}
                    >
                      Cancel
                    </Button>
                    <Button
                      variant="destructive"
                      size="sm"
                      disabled={deleteSession.isPending}
                      onClick={() => {
                        deleteSession.mutate(session.id);
                        setConfirmDeleteId(null);
                      }}
                    >
                      Delete
                    </Button>
                  </div>
                </PopoverContent>
              </Popover>
            </div>
          ))
        )}
      </div>
    </div>
  );
}
