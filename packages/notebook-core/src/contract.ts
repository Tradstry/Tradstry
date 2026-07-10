/**
 * The Y.Doc contract. Every client and the server-side projector must agree on
 * these exactly, or one side's document projects as wrong or empty — silently.
 */
export const DOC_ID = "notebook-note";
export const NAMESPACE = "tradstry-notebook";

/**
 * Yjs update blobs are raw bytes; base64 is the only representation allowed to
 * cross a JSON/stdio boundary. Standard alphabet, padded, on every side.
 *
 * Chunked because `String.fromCharCode(...bytes)` overflows the stack once a
 * document has any real edit history.
 */
const CHUNK = 0x8000;

export function toBase64(bytes: Uint8Array): string {
  let binary = "";
  for (let i = 0; i < bytes.length; i += CHUNK) {
    binary += String.fromCharCode.apply(null, [...bytes.subarray(i, i + CHUNK)]);
  }
  return btoa(binary);
}

export function fromBase64(b64: string): Uint8Array {
  const binary = atob(b64);
  const out = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i += 1) out[i] = binary.charCodeAt(i);
  return out;
}
