import { randomUUID } from "node:crypto";

type Emit = (
  id: string,
  message:
    | { type: "data"; data: unknown }
    | { type: "error"; message: string }
    | { type: "complete" },
) => void;

export function graphqlSubscriptionError(payload: unknown): string | null {
  let errors: unknown[] = [];
  if (Array.isArray(payload)) {
    errors = payload;
  } else if (payload && typeof payload === "object") {
    const value = payload as { errors?: unknown; message?: unknown };
    if (Array.isArray(value.errors)) {
      errors = value.errors;
    } else if (typeof value.message === "string") {
      return value.message;
    }
  }
  if (errors.length === 0) return null;
  const messages = errors
    .map((error) => {
      if (!error || typeof error !== "object") return null;
      const message = (error as { message?: unknown }).message;
      return typeof message === "string" ? message : null;
    })
    .filter((message): message is string => Boolean(message));
  return messages.length > 0
    ? messages.join("; ")
    : "GraphQL subscription failed";
}

export class GraphqlSubscriptions {
  readonly #endpoint: string;
  readonly #getAccessToken: () => Promise<string | null>;
  readonly #emit: Emit;
  readonly #sockets = new Map<string, WebSocket>();

  constructor(options: {
    endpoint: string;
    getAccessToken: () => Promise<string | null>;
    emit: Emit;
  }) {
    this.#endpoint = options.endpoint;
    this.#getAccessToken = options.getAccessToken;
    this.#emit = options.emit;
  }

  async subscribe(
    rendererId: string,
    query: string,
    variables?: Record<string, unknown>,
  ): Promise<void> {
    try {
      this.unsubscribe(rendererId);
      const token = await this.#getAccessToken();
      if (!token) throw new Error("Not signed in");
      const url = new URL(this.#endpoint);
      url.pathname = "/graphql/ws";
      url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
      const operationId = randomUUID();
      const socket = new WebSocket(url, "graphql-transport-ws");
      let finished = false;
      this.#sockets.set(rendererId, socket);

      const fail = (message: string) => {
        if (finished) return;
        finished = true;
        this.#emit(rendererId, { type: "error", message });
        if (this.#sockets.get(rendererId) === socket) {
          this.unsubscribe(rendererId);
        } else {
          socket.close();
        }
      };

      socket.addEventListener("open", () => {
        socket.send(JSON.stringify({
          type: "connection_init",
          payload: { authorization: `Bearer ${token}` },
        }));
      });
      socket.addEventListener("message", (event) => {
        if (finished) return;
        let message: { type: string; payload?: unknown; id?: string };
        try {
          message = JSON.parse(String(event.data)) as typeof message;
        } catch {
          fail("GraphQL subscription returned an invalid message");
          return;
        }
        if (message.type === "connection_ack") {
          socket.send(JSON.stringify({
            id: operationId,
            type: "subscribe",
            payload: { query, variables },
          }));
        } else if (message.type === "ping") {
          socket.send(JSON.stringify({ type: "pong" }));
        } else if (message.type === "next") {
          const error = graphqlSubscriptionError(message.payload);
          if (error) {
            fail(error);
            return;
          }
          const payload = message.payload && typeof message.payload === "object"
            ? message.payload as { data?: unknown }
            : null;
          if (!payload || payload.data == null) {
            fail("GraphQL subscription returned no data");
            return;
          }
          this.#emit(rendererId, { type: "data", data: payload.data });
        } else if (message.type === "error") {
          fail(graphqlSubscriptionError(message.payload) ?? "GraphQL subscription failed");
        } else if (message.type === "complete") {
          finished = true;
          this.#emit(rendererId, { type: "complete" });
          this.unsubscribe(rendererId);
        }
      });
      socket.addEventListener("error", () => {
        fail("WebSocket connection error");
      });
      socket.addEventListener("close", () => {
        if (this.#sockets.get(rendererId) === socket) {
          this.#sockets.delete(rendererId);
        }
      });
    } catch (error) {
      this.#emit(rendererId, {
        type: "error",
        message: error instanceof Error ? error.message : String(error),
      });
    }
  }

  unsubscribe(id: string): void {
    this.#sockets.get(id)?.close();
    this.#sockets.delete(id);
  }

  close(): void {
    for (const socket of this.#sockets.values()) socket.close();
    this.#sockets.clear();
  }
}
