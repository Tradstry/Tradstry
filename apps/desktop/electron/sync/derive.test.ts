import assert from "node:assert/strict";
import test from "node:test";
import { deriveMetrics, parseFlexibleDatetime } from "./derive.ts";

test("derives long and short metrics with Rust parity", () => {
  assert.deepEqual(
    deriveMetrics(100, 110, "long", "2026-01-01T00:00:00Z", "2026-01-01T01:00:00Z", null),
    { status: "profit", totalPl: 10, netRoi: 10, duration: 3600, riskReward: null },
  );
  assert.deepEqual(
    deriveMetrics(100, 110, "short", "2026-01-01T00:00:00Z", "2026-01-01T00:30:00Z", 105),
    { status: "loss", totalPl: -10, netRoi: -10, duration: 1800, riskReward: -2 },
  );
});

test("parses naive timestamps as UTC and rejects impossible dates", () => {
  assert.equal(parseFlexibleDatetime("2026-01-01"), Date.UTC(2026, 0, 1));
  assert.equal(parseFlexibleDatetime("2026-01-01 10:20"), Date.UTC(2026, 0, 1, 10, 20));
  assert.throws(() => parseFlexibleDatetime("2026-02-31"), /Invalid datetime/);
});

test("validates trade direction, close time, and stop placement", () => {
  assert.throws(() => deriveMetrics(1, 2, "flat", "2026-01-01", "2026-01-02", null), /Unsupported/);
  assert.throws(() => deriveMetrics(1, 2, "long", "2026-01-02", "2026-01-01", null), /close_date/);
  assert.throws(() => deriveMetrics(100, 110, "long", "2026-01-01", "2026-01-02", 101), /stop_loss/);
});
