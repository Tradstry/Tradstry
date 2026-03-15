import {
  getBackendWebSocketUrl,
  type GraphQLFetcher,
} from "@/lib/client";
import type {
  AiChatCompletedEvent,
  AiChatMessageRecord,
  AiChatMessageInput,
  AiChatStreamEvent,
  AiChatStreamHandlers,
  AiChatStreamResult,
  AiChatThread,
  DeleteAiChatThreadResult,
} from "@/lib/types/ai_chat";

const GRAPHQL_WS_PROTOCOL = "graphql-transport-ws";
const GRAPHQL_WS_SUBSCRIPTION_ID = "1";

const AI_CHAT_STREAM_SUBSCRIPTION = `
  subscription AiChatStream($input: AiChatInput!) {
    aiChatStream(input: $input) {
      event
      requestId
      threadId
      text
      promotedMemoryUris
      message
    }
  }
`;

const AI_CHAT_MUTATION = `
  mutation AiChat($input: AiChatInput!) {
    aiChat(input: $input) {
      requestId
      threadId
      text
      promotedMemoryUris
    }
  }
`;

const DELETE_AI_CHAT_THREAD_MUTATION = `
  mutation DeleteAiChatThread($input: DeleteAiChatThreadInput!) {
    deleteAiChatThread(input: $input) {
      success
    }
  }
`;

const AI_CHAT_THREADS_QUERY = `
  query AiChatThreads {
    aiChatThreads {
      id
      title
      createdAt
      updatedAt
    }
  }
`;

const AI_CHAT_MESSAGES_QUERY = `
  query AiChatMessages($threadId: String!) {
    aiChatMessages(threadId: $threadId) {
      id
      threadId
      requestId
      role
      content
      createdAt
    }
  }
`;

type TokenProvider = () => Promise<string | null>;

interface GraphQLWsNextPayload {
  data?: {
    aiChatStream?: {
      event: AiChatStreamEvent["event"];
      requestId: string;
      threadId: string;
      text: string;
      promotedMemoryUris: string[];
      message: string;
    };
  };
  errors?: Array<{ message: string }>;
}

interface GraphQLWsMessage {
  type: string;
  id?: string;
  payload?: GraphQLWsNextPayload | { message?: string; errors?: Array<{ message: string }> };
}

function parseWebSocketMessage(event: MessageEvent): GraphQLWsMessage {
  if (typeof event.data === "string") {
    return JSON.parse(event.data) as GraphQLWsMessage;
  }
  if (event.data instanceof ArrayBuffer) {
    return JSON.parse(new TextDecoder().decode(event.data)) as GraphQLWsMessage;
  }
  throw new Error("Unsupported websocket payload format");
}

function extractErrorMessage(payload?: {
  message?: string;
  errors?: Array<{ message: string }>;
}): string {
  if (payload?.message) {
    return payload.message;
  }
  if (payload?.errors?.length) {
    return payload.errors[0]?.message || "AI chat stream failed";
  }
  return "AI chat stream failed";
}

function applyStreamEvent(
  streamEvent: {
    event: AiChatStreamEvent["event"];
    requestId: string;
    threadId: string;
    text: string;
    promotedMemoryUris: string[];
    message: string;
  },
  handlers: AiChatStreamHandlers,
  result: AiChatStreamResult,
): boolean {
  if (!result.requestId) {
    result.requestId = streamEvent.requestId;
    result.threadId = streamEvent.threadId;
  }

  if (streamEvent.event === "delta") {
    result.text += streamEvent.text;
    handlers.onDelta?.(streamEvent.text);
    return false;
  }

  if (streamEvent.event === "completed") {
    result.requestId = streamEvent.requestId;
    result.threadId = streamEvent.threadId;
    result.text = streamEvent.text;
    result.promotedMemoryUris = streamEvent.promotedMemoryUris;
    handlers.onCompleted?.({
      event: "completed",
      requestId: streamEvent.requestId,
      threadId: streamEvent.threadId,
      text: streamEvent.text,
      promotedMemoryUris: streamEvent.promotedMemoryUris,
    } as AiChatCompletedEvent);
    return true;
  }

  if (streamEvent.event === "error") {
    throw new Error(streamEvent.message || "AI chat stream error");
  }

  throw new Error(`Unknown AI chat event: ${streamEvent.event}`);
}

export async function streamAiChat(
  getToken: TokenProvider,
  input: AiChatMessageInput,
  handlers: AiChatStreamHandlers = {},
  signal?: AbortSignal,
): Promise<AiChatStreamResult> {
  const token = await getToken();
  const socket = new WebSocket(getBackendWebSocketUrl(), GRAPHQL_WS_PROTOCOL);
  socket.binaryType = "arraybuffer";

  const result: AiChatStreamResult = {
    requestId: "",
    threadId: "",
    text: "",
    promotedMemoryUris: [],
  };

  let started = false;
  let settled = false;
  let completedPayloadReceived = false;
  let resolvePromise: (value: AiChatStreamResult) => void = () => {};
  let rejectPromise: (error: Error) => void = () => {};

  const closeSocket = () => {
    if (socket.readyState === WebSocket.OPEN || socket.readyState === WebSocket.CONNECTING) {
      socket.close();
    }
  };

  const cleanup = () => {
    socket.removeEventListener("open", onOpen);
    socket.removeEventListener("message", onMessage);
    socket.removeEventListener("error", onError);
    socket.removeEventListener("close", onClose);
    if (signal) {
      signal.removeEventListener("abort", onAbort);
    }
  };

  const resolveStream = () => {
    if (settled) {
      return;
    }
    settled = true;
    cleanup();
    resolvePromise(result);
    closeSocket();
  };

  const rejectStream = (error: Error) => {
    if (settled) {
      return;
    }
    settled = true;
    handlers.onError?.(error.message);
    cleanup();
    rejectPromise(error);
    closeSocket();
  };

  const sendSubscriptionStart = () => {
    socket.send(
      JSON.stringify({
        id: GRAPHQL_WS_SUBSCRIPTION_ID,
        type: "start",
        payload: {
          query: AI_CHAT_STREAM_SUBSCRIPTION,
          variables: {
            input: {
              message: input.message,
              threadId: input.threadId ?? null,
            },
          },
        },
      }),
    );
  };

  const sendSubscriptionStop = () => {
    socket.send(
      JSON.stringify({
        id: GRAPHQL_WS_SUBSCRIPTION_ID,
        type: "complete",
      }),
    );
  };

  function onOpen() {
    socket.send(
      JSON.stringify({
        type: "connection_init",
        payload: token ? { authorization: `Bearer ${token}` } : {},
      }),
    );
  }

  function onMessage(event: MessageEvent) {
    let message: GraphQLWsMessage;
    try {
      message = parseWebSocketMessage(event);
    } catch (error) {
      rejectStream(error instanceof Error ? error : new Error("Invalid stream payload"));
      return;
    }

    if (message.type === "connection_ack") {
      if (!started) {
        sendSubscriptionStart();
        started = true;
      }
      return;
    }

    if (message.type === "next" || message.type === "data") {
      const payload = message.payload as GraphQLWsNextPayload | undefined;
      if (payload?.errors?.length) {
        rejectStream(new Error(extractErrorMessage(payload)));
        return;
      }

      const streamEvent = payload?.data?.aiChatStream;
      if (!streamEvent) {
        return;
      }

      try {
        completedPayloadReceived =
          applyStreamEvent(streamEvent, handlers, result) || completedPayloadReceived;
        if (completedPayloadReceived) {
          resolveStream();
          return;
        }
      } catch (error) {
        rejectStream(error instanceof Error ? error : new Error("AI chat stream failed"));
      }
      return;
    }

    if (message.type === "complete") {
      if (completedPayloadReceived) {
        resolveStream();
      } else {
        rejectStream(new Error("AI chat stream completed without final event"));
      }
      return;
    }

    if (message.type === "error" || message.type === "connection_error") {
      rejectStream(
        new Error(
          extractErrorMessage(message.payload as { message?: string; errors?: Array<{ message: string }> }),
        ),
      );
      return;
    }
  }

  function onError() {
    rejectStream(new Error("AI chat websocket error"));
  }

  function onClose() {
    if (!settled) {
      if (completedPayloadReceived) {
        resolveStream();
      } else {
        rejectStream(new Error("AI chat websocket closed before completion"));
      }
    }
  }

  function onAbort() {
    if (settled) {
      return;
    }
    if (started && socket.readyState === WebSocket.OPEN) {
      sendSubscriptionStop();
    }
    rejectStream(new Error("AI chat stream cancelled"));
  }

  if (signal) {
    if (signal.aborted) {
      throw new Error("AI chat stream cancelled");
    }
  }

  return new Promise<AiChatStreamResult>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;

    if (signal) {
      signal.addEventListener("abort", onAbort);
    }

    socket.addEventListener("open", onOpen);
    socket.addEventListener("message", onMessage);
    socket.addEventListener("error", onError);
    socket.addEventListener("close", onClose);
  });
}

export async function sendAiChatMutation(
  fetcher: GraphQLFetcher,
  input: AiChatMessageInput,
): Promise<AiChatStreamResult> {
  const data = await fetcher<{ aiChat: AiChatStreamResult }>(AI_CHAT_MUTATION, {
    input: { message: input.message, threadId: input.threadId ?? null },
  });
  return data.aiChat;
}

export async function deleteAiChatThread(
  fetcher: GraphQLFetcher,
  threadId: string,
): Promise<DeleteAiChatThreadResult> {
  const data = await fetcher<{ deleteAiChatThread: DeleteAiChatThreadResult }>(
    DELETE_AI_CHAT_THREAD_MUTATION,
    {
      input: { threadId },
    },
  );

  return data.deleteAiChatThread;
}

export async function fetchAiChatThreads(
  fetcher: GraphQLFetcher,
): Promise<AiChatThread[]> {
  const data = await fetcher<{ aiChatThreads: AiChatThread[] }>(AI_CHAT_THREADS_QUERY);
  return data.aiChatThreads;
}

export async function fetchAiChatMessages(
  fetcher: GraphQLFetcher,
  threadId: string,
): Promise<AiChatMessageRecord[]> {
  const data = await fetcher<{ aiChatMessages: AiChatMessageRecord[] }>(
    AI_CHAT_MESSAGES_QUERY,
    { threadId },
  );
  return data.aiChatMessages;
}
