import { expect, test } from "bun:test";
import { pendingQueue } from "../src/pending-queue";

function fakeStorage(): Storage {
  const map = new Map<string, string>();
  return {
    getItem: (key: string) => map.get(key) ?? null,
    setItem: (key: string, value: string) => {
      map.set(key, value);
    },
    removeItem: (key: string) => {
      map.delete(key);
    },
    clear: () => map.clear(),
    key: (index: number) => Array.from(map.keys())[index] ?? null,
    get length() {
      return map.size;
    },
  } as Storage;
}

test("a pushed update survives a fresh pendingQueue over the same storage", () => {
  const storage = fakeStorage();
  const update = new Uint8Array([1, 2, 3, 4]);

  pendingQueue(storage, "note-1").push(update);

  const drained = pendingQueue(storage, "note-1").drain();
  expect(drained).toHaveLength(1);
  expect(Array.from(drained[0])).toEqual([1, 2, 3, 4]);
});

test("clear() empties the queue", () => {
  const storage = fakeStorage();
  const queue = pendingQueue(storage, "note-1");

  queue.push(new Uint8Array([9]));
  expect(queue.drain()).toHaveLength(1);

  queue.clear();
  expect(queue.drain()).toHaveLength(0);
});

test("two different noteIds do not interfere", () => {
  const storage = fakeStorage();

  pendingQueue(storage, "note-1").push(new Uint8Array([1]));
  pendingQueue(storage, "note-2").push(new Uint8Array([2]));

  expect(pendingQueue(storage, "note-1").drain()).toHaveLength(1);
  expect(pendingQueue(storage, "note-2").drain()).toHaveLength(1);

  pendingQueue(storage, "note-1").clear();
  expect(pendingQueue(storage, "note-1").drain()).toHaveLength(0);
  expect(pendingQueue(storage, "note-2").drain()).toHaveLength(1);
});

test("corrupt stored JSON yields [] rather than throwing", () => {
  const storage = fakeStorage();
  storage.setItem("tradstry-notebook-pending:note-1", "{not json");

  expect(() => pendingQueue(storage, "note-1").drain()).not.toThrow();
  expect(pendingQueue(storage, "note-1").drain()).toEqual([]);
});

test("a large update round-trips without a stack overflow", () => {
  const storage = fakeStorage();
  const big = new Uint8Array(100_000);
  for (let i = 0; i < big.length; i += 1) big[i] = i % 256;

  expect(() => pendingQueue(storage, "note-1").push(big)).not.toThrow();

  const drained = pendingQueue(storage, "note-1").drain();
  expect(drained).toHaveLength(1);
  expect(drained[0].length).toBe(100_000);
  expect(Array.from(drained[0])).toEqual(Array.from(big));
});

test("replace() overwrites the queue", () => {
  const storage = fakeStorage();
  const queue = pendingQueue(storage, "note-1");

  queue.push(new Uint8Array([1]));
  queue.replace([new Uint8Array([5]), new Uint8Array([6])]);

  const drained = queue.drain();
  expect(drained).toHaveLength(2);
  expect(Array.from(drained[0])).toEqual([5]);
  expect(Array.from(drained[1])).toEqual([6]);
});
