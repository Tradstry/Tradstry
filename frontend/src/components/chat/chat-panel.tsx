"use client"

import * as React from "react"
import { HugeiconsIcon } from "@hugeicons/react"
import { Cancel01Icon } from "@hugeicons/core-free-icons"
import { Button } from "@/components/ui/button"
import { useChatStore, useChatSessions } from "@/hooks/chat"
import { useActiveAccount } from "@/components/accounts"
import { ChatMessageList } from "./chat-message-list"
import { ChatInput } from "./chat-input"
import { ChatSessionList } from "./chat-session-list"

const CHAT_WIDTH = 380

// ── Context so any component can check if chat panel is open ──

const ChatContext = React.createContext<{
  open: boolean
  toggle: () => void
}>({ open: false, toggle: () => {} })

export function useChatPanel() {
  return React.useContext(ChatContext)
}

// ── ChatProvider: wraps children with flex layout + margin shift ──

export function ChatProvider({ children }: { children: React.ReactNode }) {
  const open = useChatStore((s) => s.isOpen)
  const toggle = useChatStore((s) => s.toggleOpen)

  return (
    <ChatContext.Provider value={{ open, toggle }}>
      <div className="flex h-full w-full">
        <div className="min-w-0 flex-1">{children}</div>
        {open && (
          <aside
            aria-label="Chat AI"
            className="flex shrink-0 flex-col border-l border-border/60 bg-background animate-in slide-in-from-right duration-200"
            style={{ width: CHAT_WIDTH }}
          >
            <ChatPanelContent />
          </aside>
        )}
      </div>
    </ChatContext.Provider>
  )
}

// ── The actual chat panel content ──

function ChatPanelContent() {
  const activeAccount = useActiveAccount()
  const accountId = activeAccount?.id ?? ""
  const { activeSessionId, setActiveSession, setOpen } = useChatStore()
  const { data: sessions = [] } = useChatSessions(accountId)

  const activeSession = sessions.find((s) => s.id === activeSessionId)

  function handleBack() {
    setActiveSession(null)
  }

  return (
    <>
      {/* Header */}
      <div className="flex shrink-0 items-center justify-between border-b border-border px-4 py-3">
        <div className="flex items-center gap-2">
          {activeSessionId && (
            <button
              onClick={handleBack}
              className="text-muted-foreground transition-colors hover:text-foreground"
              aria-label="Back to sessions"
            >
              <svg
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
          )}
          <div>
            <h2 className="text-sm font-semibold">
              {activeSessionId
                ? (activeSession?.title ?? "Chat")
                : "Chat AI"}
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
          <ChatInput sessionId={activeSessionId} accountId={accountId} />
        </>
      ) : (
        <ChatSessionList accountId={accountId} />
      )}
    </>
  )
}
