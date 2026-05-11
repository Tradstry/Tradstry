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
