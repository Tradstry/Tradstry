"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { create } from "zustand";
import { useGraphQL, useGraphQLSubscription } from "@/lib/client";
import * as chatService from "@/lib/service/chat";
import type { ChatContext, ChatMessage, ChatSession } from "@/lib/types/chat";
import type { ChatStreamEvent } from "@/lib/types/chat";

export type ThinkingStep = {
  toolName: string;
  args: string | null;
  result: string | null;
  status: "running" | "done";
};

// ---------------------------------------------------------------------------
// Zustand store
// ---------------------------------------------------------------------------

interface ChatStore {
  isOpen: boolean;
  activeSessionId: string | null;
  pinnedContext: ChatContext;
  streamingMessage: string;
  reasoningText: string;
  thinkingSteps: ThinkingStep[];
  isStreaming: boolean;
  optimisticUserMessage: string | null;
  streamError: string | null;
  lastFailedMessage: { sessionId: string; content: string; context?: ChatContext } | null;
  // actions
  setOpen: (open: boolean) => void;
  toggleOpen: () => void;
  setActiveSession: (id: string | null) => void;
  setPinnedContext: (ctx: ChatContext) => void;
  clearPinnedContext: () => void;
  appendStreamToken: (token: string) => void;
  appendReasoningToken: (token: string) => void;
  addThinkingStep: (toolName: string, args: string | null) => void;
  completeThinkingStep: (toolName: string, result: string | null) => void;
  startStreaming: () => void;
  stopStreaming: () => void;
  resetStream: () => void;
  setOptimisticUserMessage: (msg: string | null) => void;
  setStreamError: (error: string | null) => void;
  setLastFailedMessage: (msg: { sessionId: string; content: string; context?: ChatContext } | null) => void;
  clearError: () => void;
}

export const useChatStore = create<ChatStore>((set) => ({
  isOpen: false,
  activeSessionId: null,
  pinnedContext: {},
  streamingMessage: "",
  reasoningText: "",
  thinkingSteps: [],
  isStreaming: false,
  optimisticUserMessage: null,
  streamError: null,
  lastFailedMessage: null,

  setOpen: (open) => set({ isOpen: open }),
  toggleOpen: () => set((s) => ({ isOpen: !s.isOpen })),
  setActiveSession: (id) => set({ activeSessionId: id, optimisticUserMessage: null, isStreaming: false, streamingMessage: "", reasoningText: "", thinkingSteps: [], streamError: null, lastFailedMessage: null }),
  setPinnedContext: (ctx) => set({ pinnedContext: ctx }),
  clearPinnedContext: () => set({ pinnedContext: {} }),
  appendStreamToken: (token) =>
    set((s) => ({ streamingMessage: s.streamingMessage + token })),
  appendReasoningToken: (token) =>
    set((s) => ({ reasoningText: s.reasoningText + token })),
  addThinkingStep: (toolName, args) =>
    set((s) => ({
      thinkingSteps: [...s.thinkingSteps, { toolName, args, result: null, status: "running" }],
    })),
  completeThinkingStep: (toolName, result) =>
    set((s) => ({
      thinkingSteps: s.thinkingSteps.map((step) =>
        step.toolName === toolName && step.status === "running"
          ? { ...step, result, status: "done" }
          : step
      ),
    })),
  startStreaming: () => set({ isStreaming: true, streamingMessage: "", reasoningText: "", thinkingSteps: [] }),
  stopStreaming: () => set({ isStreaming: false, optimisticUserMessage: null }),
  resetStream: () =>
    set({ isStreaming: false, streamingMessage: "", reasoningText: "", thinkingSteps: [], optimisticUserMessage: null, streamError: null, lastFailedMessage: null }),
  setOptimisticUserMessage: (msg) => set({ optimisticUserMessage: msg }),
  setStreamError: (error) => set({ streamError: error }),
  setLastFailedMessage: (msg) => set({ lastFailedMessage: msg }),
  clearError: () => set({ streamError: null, lastFailedMessage: null }),
}));

// ---------------------------------------------------------------------------
// Query keys
// ---------------------------------------------------------------------------

function chatSessionsKey(accountId: string | null) {
  return ["chatSessions", accountId] as const;
}

function chatMessagesKey(sessionId: string | null) {
  return ["chatMessages", sessionId] as const;
}

// ---------------------------------------------------------------------------
// React Query hooks
// ---------------------------------------------------------------------------

export function useChatSessions(accountId: string | null) {
  const fetcher = useGraphQL();

  return useQuery<ChatSession[]>({
    queryKey: chatSessionsKey(accountId),
    queryFn: () => chatService.fetchChatSessions(fetcher, accountId!),
    enabled: !!accountId,
  });
}

export function useChatMessages(sessionId: string | null) {
  const fetcher = useGraphQL();

  return useQuery<ChatMessage[]>({
    queryKey: chatMessagesKey(sessionId),
    queryFn: () => chatService.fetchChatMessages(fetcher, sessionId!),
    enabled: !!sessionId,
  });
}

export function useCreateSession(accountId: string | null) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const store = useChatStore();

  return useMutation<ChatSession, Error, void>({
    mutationFn: () => chatService.createChatSession(fetcher, accountId!),
    onSuccess: (session) => {
      queryClient.invalidateQueries({ queryKey: chatSessionsKey(accountId) });
      store.setActiveSession(session.id);
    },
  });
}

export function useDeleteSession(accountId: string | null) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();
  const store = useChatStore();

  return useMutation<void, Error, string>({
    mutationFn: (sessionId) => chatService.deleteChatSession(fetcher, sessionId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: chatSessionsKey(accountId) });
      store.setActiveSession(null);
    },
  });
}

export function useSendMessage(accountId: string | null) {
  const fetcher = useGraphQL();
  const subscriber = useGraphQLSubscription();
  const queryClient = useQueryClient();
  const store = useChatStore();

  return useMutation<
    string,
    Error,
    { sessionId: string; content: string; context?: ChatContext }
  >({
    mutationFn: ({ sessionId, content, context }) =>
      chatService.sendChatMessage(fetcher, sessionId, content, context),
    onMutate: ({ content, sessionId, context }) => {
      store.setOptimisticUserMessage(content);
      store.setLastFailedMessage({ sessionId, content, context });
      store.setStreamError(null);
    },
    onSuccess: (jobId, { sessionId }) => {
      store.startStreaming();

      subscriber<{ chatStream: ChatStreamEvent }>(
        chatService.CHAT_STREAM_SUBSCRIPTION,
        { jobId },
        {
          onMessage: (data) => {
            const event = data.chatStream;
            switch (event.kind) {
              case "token":
                if (event.content) {
                  store.appendStreamToken(event.content);
                }
                break;
              case "reasoning":
                if (event.content) {
                  store.appendReasoningToken(event.content);
                }
                break;
              case "tool_start":
                if (event.toolName) {
                  store.addThinkingStep(event.toolName, event.content ?? null);
                }
                break;
              case "tool_result":
                if (event.toolName) {
                  store.completeThinkingStep(event.toolName, event.content ?? null);
                }
                break;
              case "done": {
                // Promote the in-memory stream into the React Query cache
                // BEFORE invalidating. This way the assistant response stays
                // continuously visible — first as streaming tokens, then as a
                // cached message — instead of vanishing in the window between
                // Done arriving and the server refetch completing.
                const state = useChatStore.getState();
                const userContent = state.optimisticUserMessage;
                const assistantContent = state.streamingMessage;
                const now = new Date().toISOString();
                const stamp = Date.now();

                queryClient.setQueryData<ChatMessage[]>(
                  chatMessagesKey(sessionId),
                  (old = []) => {
                    const additions: ChatMessage[] = [];
                    if (userContent) {
                      additions.push({
                        id: `${sessionId}-pending-user-${stamp}`,
                        sessionId,
                        role: "user",
                        content: userContent,
                        contextJson: null,
                        toolName: null,
                        createdAt: now,
                      });
                    }
                    if (assistantContent) {
                      additions.push({
                        id: `${sessionId}-pending-assistant-${stamp}`,
                        sessionId,
                        role: "assistant",
                        content: assistantContent,
                        contextJson: null,
                        toolName: null,
                        createdAt: now,
                      });
                    }
                    return [...old, ...additions];
                  },
                );

                store.stopStreaming();
                store.setLastFailedMessage(null);

                // Confirm with the server in the background. The refetch
                // replaces the optimistic entries with the canonical ones
                // read from the checkpoint.
                queryClient.invalidateQueries({
                  queryKey: chatMessagesKey(sessionId),
                });
                queryClient.invalidateQueries({
                  queryKey: chatSessionsKey(accountId),
                });
                break;
              }
              case "error":
                store.setStreamError(
                  event.content || "Something went wrong. Please try again."
                );
                store.stopStreaming();
                break;
            }
          },
          onError: (error) => {
            store.setStreamError(
              error.message || "Connection lost. Please try again."
            );
            store.stopStreaming();
          },
        },
      );
    },
    onError: (error) => {
      store.setStreamError(
        error.message || "Failed to send message. Please try again."
      );
      store.stopStreaming();
      store.setOptimisticUserMessage(null);
    },
  });
}
