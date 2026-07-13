// Browser-only media helpers. NEVER import this from the main barrel (`index.ts`):
// the projector bundles that entry and must stay free of DOM / Web Crypto / canvas.
// Exposed as the `@tradstry/notebook-core/media` subpath so the projector never
// pulls it in.

export const MAX_IMAGE_BYTES = 10 * 1024 * 1024;
export const MAX_VIDEO_BYTES = 250 * 1024 * 1024;
const THUMB_MAX = 640;

export function isImage(mime: string): boolean {
  return mime.startsWith("image/");
}

export function isVideo(mime: string): boolean {
  return mime.startsWith("video/");
}

export async function sha256Hex(
  bytes: ArrayBuffer | Uint8Array,
): Promise<string> {
  // `Uint8Array.from` always backs onto a fresh, non-shared ArrayBuffer, which
  // satisfies the stricter `BufferSource` typing `crypto.subtle.digest` wants —
  // a view straight off `bytes` can be backed by a `SharedArrayBuffer`.
  const buf = Uint8Array.from(
    bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes),
  );
  const digest = await crypto.subtle.digest("SHA-256", buf);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export async function probeDimensions(
  file: File,
): Promise<{ width: number; height: number; durationSeconds: number }> {
  const url = URL.createObjectURL(file);
  try {
    if (isVideo(file.type)) {
      const v = document.createElement("video");
      v.preload = "metadata";
      v.src = url;
      await new Promise<void>((res, rej) => {
        v.onloadedmetadata = () => res();
        v.onerror = () => rej(new Error("probe video"));
      });
      return {
        width: v.videoWidth,
        height: v.videoHeight,
        durationSeconds: v.duration || 0,
      };
    }
    const img = new Image();
    img.src = url;
    await new Promise<void>((res, rej) => {
      img.onload = () => res();
      img.onerror = () => rej(new Error("probe image"));
    });
    return { width: img.naturalWidth, height: img.naturalHeight, durationSeconds: 0 };
  } finally {
    URL.revokeObjectURL(url);
  }
}

function drawScaled(
  source: CanvasImageSource,
  w: number,
  h: number,
): Promise<Blob> {
  const scale = Math.min(1, THUMB_MAX / Math.max(w, h || 1));
  const cw = Math.max(1, Math.round(w * scale));
  const ch = Math.max(1, Math.round(h * scale));
  const canvas = document.createElement("canvas");
  canvas.width = cw;
  canvas.height = ch;
  const ctx = canvas.getContext("2d");
  if (!ctx) {
    return Promise.reject(new Error("thumbnail canvas context"));
  }
  ctx.drawImage(source, 0, 0, cw, ch);
  return new Promise((res, rej) =>
    canvas.toBlob(
      (b) => (b ? res(b) : rej(new Error("thumbnail encode"))),
      "image/jpeg",
      0.8,
    ),
  );
}

export async function makeThumbnail(file: File): Promise<Blob> {
  const url = URL.createObjectURL(file);
  try {
    if (isVideo(file.type)) {
      const v = document.createElement("video");
      v.preload = "metadata";
      v.muted = true;
      v.src = url;
      await new Promise<void>((res, rej) => {
        v.onloadedmetadata = () => res();
        v.onerror = () => rej(new Error("thumb video meta"));
      });
      await new Promise<void>((res) => {
        v.onseeked = () => res();
        v.currentTime = Math.min(0.1, v.duration || 0.1);
      });
      return drawScaled(v, v.videoWidth, v.videoHeight);
    }
    const img = new Image();
    img.src = url;
    await new Promise<void>((res, rej) => {
      img.onload = () => res();
      img.onerror = () => rej(new Error("thumb image"));
    });
    return drawScaled(img, img.naturalWidth, img.naturalHeight);
  } finally {
    URL.revokeObjectURL(url);
  }
}
