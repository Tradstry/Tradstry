export interface AiChatMessageInput {
  message: string;
  threadId?: string | null;
}

export interface AiChatThread {
  id: string;
  title: string;
  createdAt: string;
  updatedAt: string;
}

export interface AiChatMessageRecord {
  id: string;
  threadId: string;
  requestId?: string | null;
  role: string;
  content: string;
  createdAt: string;
}

export interface AiChatDeltaEvent {
  event: "delta";
  requestId: string;
  threadId: string;
  text: string;
}

export interface AiChatCompletedEvent {
  event: "completed";
  requestId: string;
  threadId: string;
  text: string;
  promotedMemoryUris: string[];
}

export interface AiChatErrorEvent {
  event: "error";
  requestId: string;
  threadId: string;
  message: string;
}

export type AiChatStreamEvent =
  | AiChatDeltaEvent
  | AiChatCompletedEvent
  | AiChatErrorEvent;

export interface AiChatStreamResult {
  requestId: string;
  threadId: string;
  text: string;
  promotedMemoryUris: string[];
}

export interface AiChatStreamHandlers {
  onDelta?: (chunk: string) => void;
  onCompleted?: (event: AiChatCompletedEvent) => void;
  onError?: (message: string) => void;
}

export interface DeleteAiChatThreadResult {
  success: boolean;
}
