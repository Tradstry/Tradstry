import type { GraphQLFetcher } from "@/lib/client";
import type {
  ChatSession,
  ChatMessage,
  ChatContext,
} from "@/lib/types/chat";

const CHAT_SESSION_FIELDS = `
  id
  title
  createdAt
  updatedAt
`;

const CHAT_MESSAGE_FIELDS = `
  id
  sessionId
  role
  content
  contextJson
  toolName
  createdAt
`;

const CHAT_SESSIONS_QUERY = `
  query ChatSessions($accountId: String!, $limit: Int) {
    chatSessions(accountId: $accountId, limit: $limit) {
      ${CHAT_SESSION_FIELDS}
    }
  }
`;

const CHAT_MESSAGES_QUERY = `
  query ChatMessages($sessionId: String!, $limit: Int, $before: String) {
    chatMessages(sessionId: $sessionId, limit: $limit, before: $before) {
      ${CHAT_MESSAGE_FIELDS}
    }
  }
`;

const CREATE_SESSION_MUTATION = `
  mutation CreateChatSession($accountId: String!) {
    createChatSession(accountId: $accountId) {
      ${CHAT_SESSION_FIELDS}
    }
  }
`;

const UPDATE_SESSION_MUTATION = `
  mutation UpdateChatSession($sessionId: String!, $title: String!) {
    updateChatSession(sessionId: $sessionId, title: $title) {
      ${CHAT_SESSION_FIELDS}
    }
  }
`;

const DELETE_SESSION_MUTATION = `
  mutation DeleteChatSession($sessionId: String!) {
    deleteChatSession(sessionId: $sessionId)
  }
`;

const SEND_MESSAGE_MUTATION = `
  mutation SendChatMessage($sessionId: String!, $content: String!, $context: ChatContextInput) {
    sendChatMessage(sessionId: $sessionId, content: $content, context: $context)
  }
`;

export const CHAT_STREAM_SUBSCRIPTION = `
  subscription ChatStream($jobId: String!) {
    chatStream(jobId: $jobId) {
      jobId
      sessionId
      kind
      content
      toolName
      messageId
    }
  }
`;

export async function fetchChatSessions(
  fetcher: GraphQLFetcher,
  accountId: string,
  limit?: number,
): Promise<ChatSession[]> {
  const data = await fetcher<{ chatSessions: ChatSession[] }>(
    CHAT_SESSIONS_QUERY,
    { accountId, limit },
  );
  return data.chatSessions;
}

export async function fetchChatMessages(
  fetcher: GraphQLFetcher,
  sessionId: string,
  limit?: number,
  before?: string,
): Promise<ChatMessage[]> {
  const data = await fetcher<{ chatMessages: ChatMessage[] }>(
    CHAT_MESSAGES_QUERY,
    { sessionId, limit, before },
  );
  return data.chatMessages;
}

export async function createChatSession(
  fetcher: GraphQLFetcher,
  accountId: string,
): Promise<ChatSession> {
  const data = await fetcher<{ createChatSession: ChatSession }>(
    CREATE_SESSION_MUTATION,
    { accountId },
  );
  return data.createChatSession;
}

export async function updateChatSession(
  fetcher: GraphQLFetcher,
  sessionId: string,
  title: string,
): Promise<ChatSession> {
  const data = await fetcher<{ updateChatSession: ChatSession }>(
    UPDATE_SESSION_MUTATION,
    { sessionId, title },
  );
  return data.updateChatSession;
}

export async function deleteChatSession(
  fetcher: GraphQLFetcher,
  sessionId: string,
): Promise<void> {
  await fetcher<{ deleteChatSession: boolean }>(DELETE_SESSION_MUTATION, {
    sessionId,
  });
}

export async function sendChatMessage(
  fetcher: GraphQLFetcher,
  sessionId: string,
  content: string,
  context?: ChatContext,
): Promise<string> {
  const data = await fetcher<{ sendChatMessage: string }>(
    SEND_MESSAGE_MUTATION,
    { sessionId, content, context },
  );
  return data.sendChatMessage;
}
