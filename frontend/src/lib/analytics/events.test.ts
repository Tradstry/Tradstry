import { expect, test } from "bun:test";
import { EVENTS } from "./events";

test("every event name is unique", () => {
  const names = Object.values(EVENTS);
  expect(new Set(names).size).toBe(names.length);
});

test("every event name is snake_case", () => {
  for (const name of Object.values(EVENTS)) {
    expect(name).toMatch(/^[a-z][a-z0-9]*(_[a-z0-9]+)*$/);
  }
});

test("catalog covers the twenty core product events", () => {
  expect(Object.keys(EVENTS).length).toBe(20);
});
