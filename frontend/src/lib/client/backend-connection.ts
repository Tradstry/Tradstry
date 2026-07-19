const BACKEND_URL =
  process.env.NEXT_PUBLIC_BACKEND_URL ?? "http://localhost:7899/graphql";

export function getBackendBaseUrl(): string {
  return BACKEND_URL.endsWith("/graphql")
    ? BACKEND_URL.slice(0, -"/graphql".length)
    : BACKEND_URL;
}

export function getBackendWebSocketUrl(): string {
  const baseUrl = new URL(getBackendBaseUrl());
  const wsUrl = new URL("/graphql/ws", baseUrl);
  wsUrl.protocol = baseUrl.protocol === "https:" ? "wss:" : "ws:";
  return wsUrl.toString();
}

export type GraphQLFetcher = <T>(
  query: string,
  variables?: Record<string, unknown>,
) => Promise<T>;

export type GraphQLSubscriptionHandlers<T> = {
  onMessage: (data: T) => void;
  onError?: (error: Error) => void;
  onComplete?: () => void;
};

export type GraphQLSubscriber = <T>(
  query: string,
  variables: Record<string, unknown> | undefined,
  handlers: GraphQLSubscriptionHandlers<T>,
) => () => void;

const DEBUG_GRAPHQL = process.env.NODE_ENV !== "production";

export function createGraphQLFetcher(
  getToken: () => Promise<string | null>,
): GraphQLFetcher {
  return async <T>(
    query: string,
    variables?: Record<string, unknown>,
  ): Promise<T> => {
    const token = await getToken();
    const operationMatch = query.match(/\b(query|mutation)\s+([A-Za-z0-9_]+)/);
    const operationName = operationMatch?.[2] ?? "anonymous";

    if (DEBUG_GRAPHQL) {
      console.debug("[graphql] sending request", {
        operationName,
        url: BACKEND_URL,
        hasToken: Boolean(token),
        variables: variables ?? null,
      });
    }

    const res = await fetch(BACKEND_URL, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/graphql-response+json, application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: JSON.stringify({ query, variables }),
    });

    if (!res.ok) {
      if (DEBUG_GRAPHQL) {
        console.error("[graphql] http error", {
          operationName,
          url: BACKEND_URL,
          status: res.status,
          statusText: res.statusText,
        });
      }
      throw new Error(
        `GraphQL request failed: ${res.status} ${res.statusText}`,
      );
    }

    const json = await res.json();
    if (json.errors?.length) {
      if (DEBUG_GRAPHQL) {
        console.error("[graphql] graphql error", {
          operationName,
          url: BACKEND_URL,
          errors: json.errors,
        });
      }
      // Carry `extensions` on the thrown error: callers need the structured
      // `code` (plan limits, rate limits) to react, not just the message.
      const error = new Error(json.errors[0].message) as Error & {
        extensions?: Record<string, unknown>;
      };
      error.extensions = json.errors[0].extensions;
      throw error;
    }

    if (DEBUG_GRAPHQL) {
      console.debug("[graphql] request succeeded", {
        operationName,
        url: BACKEND_URL,
      });
    }

    return json.data as T;
  };
}

export function createGraphQLSubscriber(
  getToken: () => Promise<string | null>,
): GraphQLSubscriber {
  return <T>(
    query: string,
    variables: Record<string, unknown> | undefined,
    handlers: GraphQLSubscriptionHandlers<T>,
  ) => {
    const operationId =
      typeof crypto !== "undefined" && "randomUUID" in crypto
        ? crypto.randomUUID()
        : `sub_${Date.now()}`;

    let socket: WebSocket | null = null;
    let closed = false;
    let acked = false;

    const closeSocket = () => {
      if (!socket) return;
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ id: operationId, type: "complete" }));
      }
      socket.close();
      socket = null;
    };

    const fail = (error: Error) => {
      if (closed) return;
      handlers.onError?.(error);
    };

    void (async () => {
      try {
        const token = await getToken();
        if (closed) return;

        socket = new WebSocket(
          getBackendWebSocketUrl(),
          "graphql-transport-ws",
        );

        socket.onopen = () => {
          socket?.send(
            JSON.stringify({
              type: "connection_init",
              payload: token ? { authorization: `Bearer ${token}` } : {},
            }),
          );
        };

        socket.onmessage = (event) => {
          try {
            const message = JSON.parse(event.data) as {
              id?: string;
              type: string;
              payload?: {
                data?: T;
                errors?: Array<{ message?: string }>;
              };
            };

            if (message.type === "connection_ack") {
              acked = true;
              socket?.send(
                JSON.stringify({
                  id: operationId,
                  type: "subscribe",
                  payload: {
                    query,
                    variables,
                  },
                }),
              );
              return;
            }

            if (message.type === "ping") {
              socket?.send(JSON.stringify({ type: "pong" }));
              return;
            }

            if (message.type === "error") {
              const text =
                message.payload?.errors?.[0]?.message ??
                "GraphQL subscription error";
              fail(new Error(text));
              closeSocket();
              return;
            }

            if (message.type === "next" && message.payload?.data) {
              handlers.onMessage(message.payload.data);
              return;
            }

            if (message.type === "complete") {
              handlers.onComplete?.();
              closeSocket();
            }
          } catch (error) {
            fail(
              error instanceof Error
                ? error
                : new Error("Failed to parse subscription payload"),
            );
          }
        };

        socket.onerror = () => {
          fail(new Error("WebSocket connection error"));
        };

        socket.onclose = () => {
          if (!closed && acked) {
            handlers.onComplete?.();
          }
        };
      } catch (error) {
        fail(
          error instanceof Error
            ? error
            : new Error("Failed to start subscription"),
        );
      }
    })();

    return () => {
      closed = true;
      closeSocket();
    };
  };
}
