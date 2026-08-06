import assert from "node:assert/strict";
import test from "node:test";
import { graphqlSubscriptionError } from "./graphql-subscriptions.ts";

test("reads errors from a GraphQL next payload with null data", () => {
  assert.equal(
    graphqlSubscriptionError({
      data: null,
      errors: [{ message: "Unable to start Polygon market stream" }],
    }),
    "Unable to start Polygon market stream",
  );
});

test("reads graphql-transport-ws error payloads", () => {
  assert.equal(
    graphqlSubscriptionError([
      { message: "Not authorized" },
      { message: "Subscription stopped" },
    ]),
    "Not authorized; Subscription stopped",
  );
  assert.equal(graphqlSubscriptionError({ data: { quote: 1 } }), null);
});
