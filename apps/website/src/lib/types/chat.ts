export interface ChatSession {
  id: string;
  title: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ChatMessage {
  id: string;
  sessionId: string;
  role: "user" | "assistant" | "tool";
  content: string;
  contextJson: string | null;
  toolName: string | null;
  createdAt: string;
}

export interface ChatStreamEvent {
  jobId: string;
  sessionId: string;
  kind: "token" | "reasoning" | "tool_start" | "tool_result" | "done" | "error";
  content: string | null;
  toolName: string | null;
  messageId: string | null;
}

export interface ChatContext {
  tradeIds?: string[];
  dateRange?: { from: string; to: string };
  playbookIds?: string[];
}

const INTERNAL_ID_PATTERN =
  /\b[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}\b/gi;

/** UI backstop for old saved answers created before backend redaction existed. */
export function redactInternalIds(text: string): string {
  return text.replace(INTERNAL_ID_PATTERN, "the tagged trade");
}

/**
 * Preserve context already attached to optimistic user messages when a
 * background server refetch swaps them for their canonical copies.
 *
 * Messages are aligned from newest to oldest so repeated prompts receive the
 * context from the correct turn rather than the first matching text.
 */
export function preserveMessageContexts(
  fresh: ChatMessage[],
  previous: ChatMessage[] | undefined,
): ChatMessage[] {
  if (!previous?.length) return fresh;

  const previousUsers = previous.filter((message) => message.role === "user");
  let previousIndex = previousUsers.length - 1;
  const recoveredContexts = new Map<number, string>();

  for (let freshIndex = fresh.length - 1; freshIndex >= 0; freshIndex -= 1) {
    const message = fresh[freshIndex];
    if (message.role !== "user") continue;

    let matchIndex = previousIndex;
    while (
      matchIndex >= 0 &&
      previousUsers[matchIndex].content !== message.content
    ) {
      matchIndex -= 1;
    }

    if (matchIndex < 0) continue;

    const previousMessage = previousUsers[matchIndex];
    if (!message.contextJson && previousMessage.contextJson) {
      recoveredContexts.set(freshIndex, previousMessage.contextJson);
    }
    previousIndex = matchIndex - 1;
  }

  if (recoveredContexts.size === 0) return fresh;

  return fresh.map((message, index) => {
    const contextJson = recoveredContexts.get(index);
    return contextJson ? { ...message, contextJson } : message;
  });
}
