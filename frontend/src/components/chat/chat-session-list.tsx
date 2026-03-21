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
  useChatStore,
} from "@/hooks/chat";

interface ChatSessionListProps {
  accountId: string;
}

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

export function ChatSessionList({ accountId }: ChatSessionListProps) {
  const { data: sessions = [] } = useChatSessions(accountId);
  const createSession = useCreateSession(accountId);
  const deleteSession = useDeleteSession(accountId);
  const { setActiveSession } = useChatStore();
  const [confirmDeleteId, setConfirmDeleteId] = useState<string | null>(null);

  return (
    <div className="flex flex-1 flex-col overflow-hidden">
      {/* New Chat button */}
      <div className="shrink-0 px-4 py-3">
        <Button
          variant="outline"
          className="w-full justify-start gap-2 rounded-lg text-sm"
          onClick={() => createSession.mutate()}
          disabled={createSession.isPending}
        >
          <HugeiconsIcon icon={Add01Icon} className="size-4" />
          New Chat
        </Button>
      </div>

      {/* Session list */}
      <div className="flex-1 overflow-y-auto px-4">
        {sessions.map((session) => (
          <div
            key={session.id}
            className="group relative flex items-center rounded-lg transition-colors hover:bg-muted"
          >
            <button
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
              <PopoverContent
                side="bottom"
                align="end"
                className="w-56 p-3"
              >
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
        ))}

        {sessions.length === 0 && (
          <div className="flex h-32 items-center justify-center text-xs text-muted-foreground">
            No conversations yet
          </div>
        )}
      </div>

      {/* View all footer */}
      {sessions.length > 0 && (
        <div className="shrink-0 border-t border-border py-3 text-center">
          <span className="text-sm text-muted-foreground">View all</span>
        </div>
      )}
    </div>
  );
}
