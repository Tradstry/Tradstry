"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { create } from "zustand";
import { useGraphQL, useGraphQLSubscription } from "@/lib/client";
import * as chatService from "@/lib/service/chat";
import type { ChatContext, ChatMessage, ChatSession } from "@/lib/types/chat";
import type { ChatStreamEvent } from "@/lib/types/chat";

// ---------------------------------------------------------------------------
// Zustand store
// ---------------------------------------------------------------------------

interface ChatStore {
  isOpen: boolean;
  activeSessionId: string | null;
  pinnedContext: ChatContext;
  streamingMessage: string;
  streamingToolName: string | null;
  isStreaming: boolean;
  optimisticUserMessage: string | null;
  // actions
  setOpen: (open: boolean) => void;
  toggleOpen: () => void;
  setActiveSession: (id: string | null) => void;
  setPinnedContext: (ctx: ChatContext) => void;
  clearPinnedContext: () => void;
  appendStreamToken: (token: string) => void;
  setStreamingTool: (name: string | null) => void;
  startStreaming: () => void;
  stopStreaming: () => void;
  resetStream: () => void;
  setOptimisticUserMessage: (msg: string | null) => void;
}

export const useChatStore = create<ChatStore>((set) => ({
  isOpen: false,
  activeSessionId: null,
  pinnedContext: {},
  streamingMessage: "",
  streamingToolName: null,
  isStreaming: false,
  optimisticUserMessage: null,

  setOpen: (open) => set({ isOpen: open }),
  toggleOpen: () => set((s) => ({ isOpen: !s.isOpen })),
  setActiveSession: (id) => set({ activeSessionId: id, optimisticUserMessage: null, isStreaming: false, streamingMessage: "", streamingToolName: null }),
  setPinnedContext: (ctx) => set({ pinnedContext: ctx }),
  clearPinnedContext: () => set({ pinnedContext: {} }),
  appendStreamToken: (token) =>
    set((s) => ({ streamingMessage: s.streamingMessage + token })),
  setStreamingTool: (name) => set({ streamingToolName: name }),
  startStreaming: () => set({ isStreaming: true, streamingMessage: "", streamingToolName: null }),
  stopStreaming: () => set({ isStreaming: false, optimisticUserMessage: null }),
  resetStream: () =>
    set({ isStreaming: false, streamingMessage: "", streamingToolName: null, optimisticUserMessage: null }),
  setOptimisticUserMessage: (msg) => set({ optimisticUserMessage: msg }),
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
    onMutate: ({ content }) => {
      store.setOptimisticUserMessage(content);
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
              case "tool_start":
                store.setStreamingTool(event.toolName ?? null);
                break;
              case "tool_result":
                store.setStreamingTool(null);
                break;
              case "done":
                Promise.all([
                  queryClient.invalidateQueries({
                    queryKey: chatMessagesKey(sessionId),
                  }),
                  queryClient.invalidateQueries({
                    queryKey: chatSessionsKey(accountId),
                  }),
                ]).then(() => {
                  store.stopStreaming();
                });
                break;
              case "error":
                store.stopStreaming();
                break;
            }
          },
          onError: () => {
            store.stopStreaming();
          },
        },
      );
    },
  });
}
