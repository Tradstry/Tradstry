"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { createGraphQLFetcher, useGraphQL } from "@/lib/client";
import * as aiChatService from "@/lib/service/ai_chat";
import type {
  AiChatMessageRecord,
  AiChatMessageInput,
  AiChatStreamResult,
  AiChatStreamHandlers,
  AiChatThread,
  DeleteAiChatThreadResult,
} from "@/lib/types/ai_chat";

const AI_CHAT_KEY = ["ai-chat"] as const;

export function useAiChatThreads() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<AiChatThread[]>({
    queryKey: [...AI_CHAT_KEY, "threads"],
    queryFn: () => aiChatService.fetchAiChatThreads(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useAiChatMessages(threadId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<AiChatMessageRecord[]>({
    queryKey: [...AI_CHAT_KEY, "messages", threadId],
    queryFn: () => {
      if (!threadId) {
        throw new Error("thread id is required");
      }

      return aiChatService.fetchAiChatMessages(fetcher, threadId);
    },
    enabled: isLoaded && isSignedIn && !!threadId,
  });
}

export type AiChatMutationVariables = {
  input: AiChatMessageInput;
  handlers?: AiChatStreamHandlers;
  signal?: AbortSignal;
};

export function useAiChat() {
  const { getToken, isLoaded, isSignedIn } = useAuth();
  const queryClient = useQueryClient();
  const mutation = useMutation<AiChatStreamResult, Error, AiChatMutationVariables>({
    mutationFn: async ({ input, handlers = {}, signal }) => {
      if (!isLoaded || !isSignedIn) {
        throw new Error("You must be signed in to use AI chat");
      }

      return aiChatService.streamAiChat(
        () => getToken(),
        input,
        handlers,
        signal,
      );
    },
    onSuccess: (result) => {
      queryClient.invalidateQueries({ queryKey: AI_CHAT_KEY });
      queryClient.invalidateQueries({
        queryKey: [...AI_CHAT_KEY, "messages", result.threadId],
      });
    },
  });

  const deleteMutation = useMutation<DeleteAiChatThreadResult, Error, string>({
    mutationFn: async (threadId) => {
      if (!isLoaded || !isSignedIn) {
        throw new Error("You must be signed in to use AI chat");
      }

      return aiChatService.deleteAiChatThread(
        createGraphQLFetcher(() => getToken()),
        threadId,
      );
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: AI_CHAT_KEY });
    },
  });

  const streamAiChat = async (
    input: AiChatMessageInput,
    handlers: AiChatStreamHandlers = {},
    signal?: AbortSignal,
  ): Promise<AiChatStreamResult> =>
    mutation.mutateAsync({
      input,
      handlers,
      signal,
    });

  const deleteThread = async (threadId: string): Promise<DeleteAiChatThreadResult> =>
    deleteMutation.mutateAsync(threadId);

  return {
    streamAiChat,
    deleteThread,
    isStreaming: mutation.isPending,
    isDeleting: deleteMutation.isPending,
    ...mutation,
    error: mutation.error?.message ?? null,
    deleteError: deleteMutation.error?.message ?? null,
  };
}
