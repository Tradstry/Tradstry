import { expect, test } from "bun:test";
import {
  getLocalBlob,
  registerLocalBlob,
  revokeLocalBlob,
} from "./media-registry";

test("registry stores and clears blob urls by hash", () => {
  registerLocalBlob("h1", "blob:abc");
  expect(getLocalBlob("h1")).toBe("blob:abc");
  revokeLocalBlob("h1");
  expect(getLocalBlob("h1")).toBeUndefined();
});
