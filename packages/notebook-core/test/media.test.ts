import { expect, test } from "bun:test";
import { sha256Hex, isImage, isVideo, MAX_IMAGE_BYTES } from "../src/media";

test("sha256Hex matches the known vector for 'abc'", async () => {
  const bytes = new TextEncoder().encode("abc");
  expect(await sha256Hex(bytes)).toBe(
    "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
  );
});

test("mime helpers", () => {
  expect(isImage("image/png")).toBe(true);
  expect(isVideo("video/mp4")).toBe(true);
  expect(isImage("video/mp4")).toBe(false);
  expect(MAX_IMAGE_BYTES).toBe(10 * 1024 * 1024);
});
