"use client";

import { Cancel01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import * as React from "react";
import { TradstryMark } from "@tradstry/app-ui/components/logo";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { useActiveWorkspace } from "@tradstry/app-ui/components/workspaces";
import { useChatSessions, useChatStore } from "@tradstry/app-ui/hooks/chat";
import { cn } from "@tradstry/app-ui/lib/utils";
import { ChatInput } from "./chat-input";
import { ChatMessageList } from "./chat-message-list";
import { ChatSessionList } from "./chat-session-list";

const CHAT_WIDTH = "min(440px, 100vw)";

// ── Context so any component can check if chat panel is open ──

const ChatContext = React.createContext<{
  open: boolean;
  toggle: () => void;
}>({ open: false, toggle: () => {} });

export function useChatPanel() {
  return React.useContext(ChatContext);
}

// ── ChatProvider: wraps children with flex layout + margin shift ──

export function ChatProvider({ children }: { children: React.ReactNode }) {
  const open = useChatStore((s) => s.isOpen);
  const toggle = useChatStore((s) => s.toggleOpen);
  const reduceMotion = useReducedMotion();

  const openResizeTransition = reduceMotion
    ? { duration: 0 }
    : { duration: 0.22, ease: [0.22, 1, 0.36, 1] as const };

  const closeResizeTransition = reduceMotion
    ? { duration: 0 }
    : { duration: 0.16, ease: [0.4, 0, 1, 1] as const };

  const openPanelTransition = reduceMotion
    ? { duration: 0 }
    : { duration: 0.2, ease: [0.22, 1, 0.36, 1] as const };

  const closePanelTransition = reduceMotion
    ? { duration: 0 }
    : { duration: 0.12, ease: [0.4, 0, 1, 1] as const };

  return (
    <ChatContext.Provider value={{ open, toggle }}>
      <div className="flex h-svh w-full overflow-hidden">
        <div className="min-w-0 flex-1">{children}</div>
        <AnimatePresence initial={false}>
          {open ? (
            <motion.aside
              key="tradstry-ai-panel"
              aria-label="Tradstry AI"
              initial={{ width: 0 }}
              animate={{
                width: CHAT_WIDTH,
                transition: openResizeTransition,
              }}
              exit={{ width: 0, transition: closeResizeTransition }}
              className="h-full min-h-0 shrink-0 overflow-hidden"
            >
              <motion.div
                initial={reduceMotion ? false : { x: 28, opacity: 0 }}
                animate={{ x: 0, opacity: 1, transition: openPanelTransition }}
                exit={{
                  x: reduceMotion ? 0 : 20,
                  opacity: reduceMotion ? 1 : 0,
                  transition: closePanelTransition,
                }}
                className="flex h-full min-h-0 flex-col overflow-hidden border-l border-border/60 bg-background"
                style={{ width: CHAT_WIDTH }}
              >
                <ChatPanelContent />
              </motion.div>
            </motion.aside>
          ) : null}
        </AnimatePresence>
      </div>
    </ChatContext.Provider>
  );
}

// ── The actual chat panel content ──

function ChatPanelContent() {
  const activeWorkspace = useActiveWorkspace();
  const workspaceId = activeWorkspace?.id ?? "";
  const { activeSessionId, setActiveSession, setOpen } = useChatStore();
  const { data: sessions = [] } = useChatSessions(workspaceId);

  const activeSession = sessions.find((s) => s.id === activeSessionId);

  function handleBack() {
    setActiveSession(null);
  }

  return (
    <>
      {/* Header */}
      <div
        className={cn(
          "flex shrink-0 items-center justify-between border-b border-border",
          activeSessionId ? "px-3 py-2" : "px-4 py-3",
        )}
      >
        <div
          className={cn(
            "flex items-center",
            activeSessionId ? "gap-1.5" : "gap-2",
          )}
        >
          {activeSessionId ? (
            <button
              type="button"
              onClick={handleBack}
              className="flex size-6 items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              aria-label="Back to sessions"
            >
              <svg
                aria-hidden="true"
                width="18"
                height="18"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M15 18l-6-6 6-6" />
              </svg>
            </button>
          ) : (
            <span className="flex size-7 items-center justify-center rounded-md bg-foreground text-background">
              <TradstryMark className="size-[18px]" />
            </span>
          )}
          <div>
            <h2 className="text-sm font-semibold">
              {activeSessionId
                ? (activeSession?.title ?? "Chat")
                : "Tradstry AI"}
            </h2>
            {!activeSessionId && (
              <p className="text-xs text-muted-foreground">
                Ask questions about your trades
              </p>
            )}
          </div>
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="h-6 w-6 shrink-0"
          onClick={() => setOpen(false)}
          aria-label="Close Tradstry AI"
        >
          <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" />
        </Button>
      </div>

      {/* Body: session list or active chat */}
      {activeSessionId ? (
        <>
          <div className="flex-1 overflow-hidden">
            <ChatMessageList sessionId={activeSessionId} />
          </div>
          <ChatInput sessionId={activeSessionId} workspaceId={workspaceId} />
        </>
      ) : (
        <ChatSessionList workspaceId={workspaceId} />
      )}
    </>
  );
}
