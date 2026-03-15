"use client";

import { FormEvent, useEffect, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";

import { ManageChats } from "@/components/ai-chat";
import { useAiChat, useAiChatMessages, useAiChatThreads } from "@/hooks/ai_chat";

type ChatRole = "user" | "assistant" | "error";

type ChatMessage = {
  id: string | number;
  role: ChatRole;
  text: string;
};

export default function AIChatPage() {
  const {
    streamAiChat,
    deleteThread,
    isStreaming,
    isDeleting,
    error: mutationError,
    deleteError,
  } = useAiChat();
  const { data: chats = [] } = useAiChatThreads();
  const [messages, setMessages] = useState<ChatMessage[]>([]);
  const [input, setInput] = useState("");
  const [threadId, setThreadId] = useState<string | null>(null);
  const [isSubmitting, setIsSubmitting] = useState(false);
  const [deletingThreadId, setDeletingThreadId] = useState<string | null>(null);
  const controllerRef = useRef<AbortController | null>(null);
  const messageId = useRef(0);
  const { data: persistedMessages = [] } = useAiChatMessages(threadId);

  const addMessage = (message: Omit<ChatMessage, "id">) =>
    setMessages((prev) => [...prev, { ...message, id: ++messageId.current }]);

  const setLastAssistantText = (updater: (text: string) => string) => {
    setMessages((prev) => {
      const next = [...prev];
      if (next.length === 0) {
        return prev;
      }

      for (let i = next.length - 1; i >= 0; i -= 1) {
        if (next[i].role === "assistant") {
          next[i] = { ...next[i], text: updater(next[i].text) };
          return next;
        }
      }
      return prev;
    });
  };

  const appendAssistantChunk = (chunk: string) => {
    setLastAssistantText((text) => text + chunk);
  };

  const appendError = (message: string) => {
    addMessage({ role: "error", text: message });
  };

  useEffect(() => {
    if (!threadId) {
      return;
    }

    setMessages(
      persistedMessages
        .filter((message) =>
          message.role === "user" ||
          message.role === "assistant" ||
          message.role === "error",
        )
        .map((message) => ({
          id: message.id,
          role: message.role as ChatRole,
          text: message.content,
        })),
    );
  }, [persistedMessages, threadId]);

  const handleSubmit = async (event: FormEvent) => {
    event.preventDefault();

    const trimmedInput = input.trim();
    if (!trimmedInput || isSubmitting || isStreaming) {
      return;
    }

    const controller = new AbortController();
    controllerRef.current = controller;

    setInput("");
    setIsSubmitting(true);

    addMessage({ role: "user", text: trimmedInput });
    addMessage({ role: "assistant", text: "" });

    try {
      const result = await streamAiChat(
        {
          message: trimmedInput,
          threadId,
        },
        {
          onDelta: appendAssistantChunk,
          onCompleted: (event) => {
            setThreadId(event.threadId);
            setLastAssistantText(() => event.text);
          },
          onError: (message) => {
            appendError(`AI stream error: ${message}`);
          },
        },
        controller.signal,
      );

      setThreadId(result.threadId);
      setLastAssistantText(() => result.text);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Something went wrong while streaming.";
      if (!messages.some((msg) => msg.role === "error" && msg.text === `AI stream error: ${message}`)) {
        appendError(`AI stream error: ${message}`);
      }
    } finally {
      setIsSubmitting(false);
      controllerRef.current = null;
    }
  };

  const handleStop = () => {
    controllerRef.current?.abort();
    appendError("AI stream cancelled");
    setIsSubmitting(false);
  };

  const handleDeleteChat = async () => {
    if (!threadId || isStreaming || isSubmitting || isDeleting) {
      return;
    }

    try {
      setDeletingThreadId(threadId);
      const result = await deleteThread(threadId);
      if (!result.success) {
        appendError("AI chat delete error: Chat thread not found.");
        return;
      }

      setThreadId(null);
      setMessages([]);
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Something went wrong while deleting the chat.";
      appendError(`AI chat delete error: ${message}`);
    } finally {
      setDeletingThreadId(null);
    }
  };

  const handleCreateChat = () => {
    if (isSubmitting || isStreaming) {
      return;
    }

    controllerRef.current?.abort();
    setThreadId(null);
    setMessages([]);
    setInput("");
  };

  const handleSelectChat = (nextThreadId: string) => {
    if (isSubmitting || isStreaming) {
      return;
    }

    setThreadId(nextThreadId);
  };

  const handleDeleteChatById = async (targetThreadId: string) => {
    if (isSubmitting || isStreaming || isDeleting) {
      return;
    }

    try {
      setDeletingThreadId(targetThreadId);
      const result = await deleteThread(targetThreadId);
      if (!result.success) {
        appendError("AI chat delete error: Chat thread not found.");
        return;
      }

      if (threadId === targetThreadId) {
        setThreadId(null);
        setMessages([]);
      }
    } catch (err) {
      const message =
        err instanceof Error ? err.message : "Something went wrong while deleting the chat.";
      appendError(`AI chat delete error: ${message}`);
    } finally {
      setDeletingThreadId(null);
    }
  };

  return (
    <>
      <section className="mt-4 space-y-4">
        <div className="flex items-start justify-between gap-4">
          <div>
            <h2 className="text-lg font-medium">AI chat</h2>
            <p className="mt-1 text-sm text-muted-foreground">
              Start a conversation with your trading assistant. Each conversation is persisted as a
              Turso-backed chat thread and reused for follow-up context.
            </p>
          </div>
          <ManageChats
            chats={chats}
            selectedThreadId={threadId}
            disabled={isSubmitting || isStreaming}
            isCreating={false}
            deletingThreadId={deletingThreadId}
            onCreateChat={handleCreateChat}
            onSelectChat={handleSelectChat}
            onDeleteChat={handleDeleteChatById}
          />
        </div>

        <div className="max-h-[26rem] overflow-y-auto rounded-md border bg-muted/40 p-4">
          {messages.length === 0 ? (
            <p className="text-sm text-muted-foreground">
              Send a message to start.
            </p>
          ) : (
            <ul className="space-y-3">
              {messages.map((message) => (
                <li
                  key={message.id}
                  className={`rounded-md p-3 ${
                    message.role === "user"
                      ? "bg-primary/10 text-right"
                      : message.role === "error"
                        ? "bg-destructive/10 text-destructive"
                        : "bg-muted text-left"
                  }`}
                >
                  <p className="text-xs font-semibold uppercase tracking-wide">
                    {message.role}
                  </p>
                  {message.role === "assistant" ? (
                    <div className="mt-2 text-sm [&_code]:rounded [&_code]:bg-black/5 [&_code]:px-1 [&_code]:py-0.5 [&_ol]:list-decimal [&_ol]:pl-5 [&_pre]:overflow-x-auto [&_pre]:rounded-md [&_pre]:bg-black [&_pre]:p-3 [&_pre]:text-white [&_ul]:list-disc [&_ul]:pl-5">
                      <ReactMarkdown>{message.text || "…"}</ReactMarkdown>
                    </div>
                  ) : (
                    <p className="mt-1 whitespace-pre-wrap text-sm">{message.text || "…"}</p>
                  )}
                </li>
              ))}
            </ul>
          )}
        </div>

        <form className="flex flex-col gap-2" onSubmit={handleSubmit}>
          <textarea
            className="min-h-[110px] rounded-md border bg-background p-3 text-sm"
            placeholder="Ask your trading assistant..."
            value={input}
            onChange={(event) => setInput(event.target.value)}
            disabled={isSubmitting || isStreaming}
          />
          <div className="flex items-center gap-2">
            <button
              type="submit"
              className="rounded-md bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-50"
              disabled={isSubmitting || isStreaming || !input.trim()}
            >
              {isSubmitting || isStreaming ? "Streaming..." : "Send"}
            </button>
            {(isSubmitting || isStreaming) && (
              <button
                type="button"
                onClick={handleStop}
                className="rounded-md border px-4 py-2 text-sm"
              >
                Cancel
              </button>
            )}
            {threadId && (
              <button
                type="button"
                onClick={handleDeleteChat}
                disabled={isSubmitting || isStreaming || isDeleting}
                className="rounded-md border px-4 py-2 text-sm disabled:opacity-50"
              >
                {isDeleting ? "Deleting..." : "Delete chat"}
              </button>
            )}
          </div>
        </form>

        {mutationError && (
          <p className="mt-2 text-sm text-destructive">Request error: {mutationError}</p>
        )}
        {deleteError && (
          <p className="mt-2 text-sm text-destructive">Delete error: {deleteError}</p>
        )}
      </section>
    </>
  );
}
