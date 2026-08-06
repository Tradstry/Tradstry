import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { createWebSocketGraphQLSubscriber } from "./client";

class FakeWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static instances: FakeWebSocket[] = [];

  readyState = FakeWebSocket.CONNECTING;
  sent: string[] = [];
  closeCalls = 0;
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onerror: ((event: Event) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  constructor(_url: string | URL, _protocols?: string | string[]) {
    FakeWebSocket.instances.push(this);
  }

  open() {
    this.readyState = FakeWebSocket.OPEN;
    this.onopen?.(new Event("open"));
  }

  receive(message: unknown) {
    this.onmessage?.({ data: JSON.stringify(message) } as MessageEvent);
  }

  send(value: string) {
    this.sent.push(value);
  }

  close() {
    this.closeCalls += 1;
    this.readyState = FakeWebSocket.CLOSED;
  }
}

const realWebSocket = globalThis.WebSocket;

beforeEach(() => {
  FakeWebSocket.instances = [];
  globalThis.WebSocket = FakeWebSocket as unknown as typeof WebSocket;
});

afterEach(() => {
  globalThis.WebSocket = realWebSocket;
});

async function connectedSocket() {
  await Promise.resolve();
  await Promise.resolve();
  const socket = FakeWebSocket.instances[0];
  expect(socket).toBeDefined();
  socket!.open();
  socket!.receive({ type: "connection_ack" });
  return socket!;
}

describe("createWebSocketGraphQLSubscriber", () => {
  test("closes after protocol completion and ignores a late socket error", async () => {
    const errors: string[] = [];
    let completions = 0;
    const subscribe = createWebSocketGraphQLSubscriber({
      endpoint: "http://localhost:7899/graphql",
      getToken: async () => "token",
    });

    subscribe("subscription Test { value }", undefined, {
      onMessage: () => {},
      onError: (error) => errors.push(error.message),
      onComplete: () => { completions += 1; },
    });

    const socket = await connectedSocket();
    const lateError = socket.onerror;
    socket.receive({ type: "complete" });
    lateError?.(new Event("error"));

    expect(completions).toBe(1);
    expect(errors).toEqual([]);
    expect(socket.closeCalls).toBe(1);
  });

  test("unsubscribe sends completion and suppresses later failures", async () => {
    const errors: string[] = [];
    const subscribe = createWebSocketGraphQLSubscriber({
      endpoint: "http://localhost:7899/graphql",
      getToken: async () => null,
    });

    const unsubscribe = subscribe("subscription Test { value }", undefined, {
      onMessage: () => {},
      onError: (error) => errors.push(error.message),
    });

    const socket = await connectedSocket();
    const lateError = socket.onerror;
    unsubscribe();
    lateError?.(new Event("error"));

    expect(socket.sent.map((value) => JSON.parse(value).type)).toEqual([
      "connection_init",
      "subscribe",
      "complete",
    ]);
    expect(errors).toEqual([]);
    expect(socket.closeCalls).toBe(1);
  });
});
