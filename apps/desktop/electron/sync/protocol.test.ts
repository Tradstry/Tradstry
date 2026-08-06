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
  assert.equal(variables?.workspaceId, "account");
  assert.equal("accountId" in (variables ?? {}), false);
});

test("workspace-scoped secondary pulls send the required workspace id", async () => {
  const calls: Array<{ query: string; variables: Record<string, unknown> }> = [];
  const protocol = new SyncProtocol(async (query, variables) => {
    calls.push({ query, variables });
    if (query.includes("pullPlaybook")) return { pullPlaybook: { cookie: null, lastMutationId: 0, playbooks: [] } };
    if (query.includes("pullTags")) return { pullTags: { cookie: null, lastMutationId: 0, categories: [], tags: [] } };
    return { pullCalculator: { cookie: null, lastMutationId: 0, rules: [], plans: [], history: [] } };
  });
  await protocol.pullPlaybook("client", "workspace", null);
  await protocol.pullTags("client", "workspace", null);
  await protocol.pullCalculator("client", "workspace", null);
  assert.deepEqual(calls.map((call) => call.variables.workspaceId), ["workspace", "workspace", "workspace"]);
  assert.ok(calls.every((call) => call.query.includes("$workspaceId: String!")));
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

test("GraphQL client reports a non-JSON backend error without masking it as a parse failure", async () => {
  const client = createGraphqlClient({
    endpoint: "https://example.test/graphql",
    getAccessToken: async () => "secret",
    fetch: async () => new Response("Error: Invalid JWT!", { status: 401 }),
  });
  await assert.rejects(
    () => client("query Test { value }", {}),
    /Backend returned 401: Error: Invalid JWT!/,
  );
});
