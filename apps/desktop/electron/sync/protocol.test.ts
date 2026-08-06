import assert from "node:assert/strict";
import test from "node:test";
import { createGraphqlClient, SyncProtocol, type GraphqlClient } from "./protocol.ts";

test("push is a network no-op for an empty outbox", async () => {
  let calls = 0;
  const protocol = new SyncProtocol(async () => {
    calls += 1;
    return {};
  });
  assert.equal(await protocol.push("client", "account", []), 0);
  assert.equal(calls, 0);
});

test("push orders mutations and preserves args as JSON strings", async () => {
  let variables: Record<string, unknown> | undefined;
  const graphql: GraphqlClient = async (_query, value) => {
    variables = value;
    return { pushNotebook: { lastMutationId: 2 } };
  };
  const protocol = new SyncProtocol(graphql);
  const ack = await protocol.push("client", "account", [
    { id: 2, name: "second", args: "{\"b\":2}", hlc: "2" },
    { id: 1, name: "first", args: "{\"a\":1}", hlc: "1" },
  ]);
  assert.equal(ack, 2);
  const input = variables?.input as { mutations: Array<{ id: number; args: string }> };
  assert.deepEqual(input.mutations.map((row) => row.id), [1, 2]);
  assert.equal(input.mutations[0]?.args, "{\"a\":1}");
});

test("pull forwards the opaque cookie unchanged", async () => {
  let variables: Record<string, unknown> | undefined;
  const protocol = new SyncProtocol(async (_query, value) => {
    variables = value;
    return { pullNotebook: { cookie: "opaque:v2", lastMutationId: 0, notes: [], folders: [] } };
  });
  const result = await protocol.pull("client", "account", "opaque:v1");
  assert.equal(variables?.cookie, "opaque:v1");
  assert.equal(result.cookie, "opaque:v2");
});

test("GraphQL client attaches the access token and reports GraphQL errors", async () => {
  const requests: RequestInit[] = [];
  const client = createGraphqlClient({
    endpoint: "https://example.test/graphql",
    getAccessToken: async () => "secret",
    fetch: async (_input, init) => {
      requests.push(init ?? {});
      return new Response(JSON.stringify({ errors: [{ message: "bad" }] }), {
        status: 200,
        headers: { "content-type": "application/json" },
      });
    },
  });
  await assert.rejects(() => client("query Test { value }", {}), /GraphQL error/);
  assert.equal((requests[0]?.headers as Record<string, string>).authorization, "Bearer secret");
});
