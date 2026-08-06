import assert from "node:assert/strict";
import test from "node:test";
import { Hlc } from "./hlc.ts";

test("HLC stays monotonic when the physical clock moves backwards", () => {
  let physical = 1_000;
  const clock = new Hlc("c1", () => physical);
  const first = clock.now();
  physical = 900;
  assert.ok(clock.now() > first);
});

test("HLC advances within one millisecond", () => {
  const clock = new Hlc("c1", () => 1_000);
  assert.ok(clock.now() < clock.now());
});

test("observing a future remote stamp advances the local clock", () => {
  const clock = new Hlc("c1", () => 1_000);
  clock.observe("000000000009999:00000:c2");
  assert.ok(clock.now() > "000000000009999:00000:c2");
});

test("client id breaks equal-clock ties", () => {
  assert.ok(new Hlc("aaa", () => 5).now() < new Hlc("bbb", () => 5).now());
});
