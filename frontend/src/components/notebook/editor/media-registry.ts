// Session-only map of content hash -> local blob: URL for media that has been
// pasted but not yet confirmed resolvable via the server (`urlFor`). Lets a
// node render immediately from the just-created object URL before the upload
// finishes, without ever storing that URL on the node itself.
const localBlobs = new Map<string, string>();

export function registerLocalBlob(hash: string, url: string): void {
  localBlobs.set(hash, url);
}

export function getLocalBlob(hash: string): string | undefined {
  return localBlobs.get(hash);
}

export function revokeLocalBlob(hash: string): void {
  const url = localBlobs.get(hash);
  if (url === undefined) {
    return;
  }
  localBlobs.delete(hash);
  URL.revokeObjectURL(url);
}
