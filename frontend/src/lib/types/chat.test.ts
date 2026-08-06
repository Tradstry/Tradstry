import { describe, expect, test } from "bun:test";
import type { ChatMessage } from "./chat";
import { preserveMessageContexts, redactInternalIds } from "./chat";

function userMessage(
  id: string,
  content: string,
  contextJson: string | null,
): ChatMessage {
  return {
    id,
    sessionId: "session-1",
    role: "user",
    content,
    contextJson,
    toolName: null,
    createdAt: "",
  };
}

describe("preserveMessageContexts", () => {
  test("keeps optimistic context when the canonical message omits it", () => {
    const contextJson = JSON.stringify({ tradeIds: ["trade-1"] });
    const previous = [userMessage("pending", "Check this trade", contextJson)];
    const fresh = [userMessage("canonical", "Check this trade", null)];

    expect(preserveMessageContexts(fresh, previous)[0].contextJson).toBe(
      contextJson,
    );
  });

  test("aligns repeated prompts from newest to oldest", () => {
    const firstContext = JSON.stringify({ tradeIds: ["trade-1"] });
    const secondContext = JSON.stringify({ tradeIds: ["trade-2"] });
    const previous = [
      userMessage("old-1", "Review this", firstContext),
      userMessage("old-2", "Review this", secondContext),
    ];
    const fresh = [
      userMessage("new-1", "Review this", null),
      userMessage("new-2", "Review this", null),
    ];

    const merged = preserveMessageContexts(fresh, previous);

    expect(merged[0].contextJson).toBe(firstContext);
    expect(merged[1].contextJson).toBe(secondContext);
  });

  test("does not overwrite context returned by the server", () => {
    const serverContext = JSON.stringify({ playbookIds: ["playbook-1"] });
    const previous = [
      userMessage(
        "pending",
        "Review this",
        JSON.stringify({ tradeIds: ["trade-1"] }),
      ),
    ];
    const fresh = [userMessage("canonical", "Review this", serverContext)];

    expect(preserveMessageContexts(fresh, previous)[0].contextJson).toBe(
      serverContext,
    );
  });
});

describe("redactInternalIds", () => {
  test("hides internal UUIDs in saved answers", () => {
    expect(
      redactInternalIds(
        "Trade 93a79073-f4ef-4cd1-8ec9-5a14b27a2593 was profitable",
      ),
    ).toBe("Trade the tagged trade was profitable");
  });

  test("leaves symbols and dates visible", () => {
    expect(redactInternalIds("SMCI long on 2026-08-06")).toBe(
      "SMCI long on 2026-08-06",
    );
  });
});
