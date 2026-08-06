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

let configuredBackendBaseUrl = "";

export function configureBackendBaseUrl(value: string): void {
  configuredBackendBaseUrl = value.replace(/\/$/, "");
}

export function getBackendBaseUrl(): string {
  if (!configuredBackendBaseUrl) {
    throw new Error("Tradstry backend URL has not been configured");
  }
  return configuredBackendBaseUrl;
}

export function createHttpGraphQLFetcher(options: {
  endpoint: string;
  getToken: () => Promise<string | null>;
}): GraphQLFetcher {
  return async <T>(query: string, variables?: Record<string, unknown>) => {
    const token = await options.getToken();
    const response = await fetch(options.endpoint, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        Accept: "application/graphql-response+json, application/json",
        ...(token ? { Authorization: `Bearer ${token}` } : {}),
      },
      body: JSON.stringify({ query, variables }),
    });
    if (!response.ok) {
      throw new Error(
        `GraphQL request failed: ${response.status} ${response.statusText}`,
      );
    }
    const payload = (await response.json()) as {
      data?: T;
      errors?: Array<{ message?: string }>;
    };
    if (payload.errors?.length) {
      throw new Error(payload.errors[0]?.message ?? "GraphQL request failed");
    }
    if (payload.data === undefined) {
      throw new Error("GraphQL response did not include data");
    }
    return payload.data;
  };
}

export function createWebSocketGraphQLSubscriber(options: {
  endpoint: string;
  getToken: () => Promise<string | null>;
}): GraphQLSubscriber {
  return <T>(query: string, variables: Record<string, unknown> | undefined, handlers: GraphQLSubscriptionHandlers<T>) => {
    const id = crypto.randomUUID();
    const url = new URL(options.endpoint);
    url.pathname = "/graphql/ws";
    url.protocol = url.protocol === "https:" ? "wss:" : "ws:";
    let socket: WebSocket | null = null;
    let closed = false;

    const closeSocket = (notifyServer: boolean) => {
      const current = socket;
      socket = null;
      if (!current) return;

      current.onerror = null;
      current.onclose = null;
      if (notifyServer && current.readyState === WebSocket.OPEN) {
        current.send(JSON.stringify({ id, type: "complete" }));
      }
      current.close();
    };

    const fail = (error: Error) => {
      if (closed) return;
      closed = true;
      closeSocket(false);
      handlers.onError?.(error);
    };

    const complete = () => {
      if (closed) return;
      closed = true;
      closeSocket(false);
      handlers.onComplete?.();
    };

    void options.getToken().then((token) => {
      if (closed) return;
      socket = new WebSocket(url, "graphql-transport-ws");
      socket.onopen = () => {
        socket?.send(JSON.stringify({
          type: "connection_init",
          payload: token ? { authorization: `Bearer ${token}` } : {},
        }));
      };
      socket.onmessage = (event) => {
        if (closed) return;
        try {
          const message = JSON.parse(String(event.data)) as {
            type: string;
            payload?: { data?: T; errors?: Array<{ message?: string }> };
          };
          if (message.type === "connection_ack") {
            socket?.send(JSON.stringify({ id, type: "subscribe", payload: { query, variables } }));
          } else if (message.type === "ping") {
            socket?.send(JSON.stringify({ type: "pong" }));
          } else if (message.type === "next" && message.payload?.data) {
            handlers.onMessage(message.payload.data);
          } else if (message.type === "error") {
            fail(new Error(message.payload?.errors?.[0]?.message ?? "GraphQL subscription failed"));
          } else if (message.type === "complete") {
            complete();
          }
        } catch (error) {
          fail(error instanceof Error ? error : new Error("Failed to parse subscription payload"));
        }
      };
      socket.onerror = () => fail(new Error("WebSocket connection error"));
      socket.onclose = (event) => {
        if (closed) return;
        if (event.wasClean) complete();
        else fail(new Error("WebSocket connection closed unexpectedly"));
      };
    }).catch((error) => fail(error instanceof Error ? error : new Error(String(error))));

    return () => {
      if (closed) return;
      closed = true;
      closeSocket(true);
    };
  };
}

export { useGraphQL, useGraphQLSubscription } from "../platform/provider";
