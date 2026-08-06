import { randomBytes } from "node:crypto";
import type { DatabaseSync } from "node:sqlite";

export function enqueueMutation(
  db: DatabaseSync,
  name: string,
  args: Record<string, unknown>,
  hlc: string,
): void {
  db.prepare("INSERT INTO outbox (name, args, hlc) VALUES (?, ?, ?)").run(name, JSON.stringify(args), hlc);
}

/** UUIDv7 keeps locally minted primary keys time-ordered for Postgres indexes. */
export function uuidV7(now = Date.now()): string {
  const bytes = randomBytes(16);
  let millis = BigInt(Math.max(0, Math.trunc(now)));
  for (let index = 5; index >= 0; index -= 1) {
    bytes[index] = Number(millis & 0xffn);
    millis >>= 8n;
  }
  bytes[6] = (bytes[6]! & 0x0f) | 0x70;
  bytes[8] = (bytes[8]! & 0x3f) | 0x80;
  const hex = bytes.toString("hex");
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export function decodeBase64Strict(value: string): Uint8Array {
  const compact = value.trim();
  if (!/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(compact)) {
    throw new Error("invalid base64");
  }
  return Buffer.from(compact, "base64");
}
