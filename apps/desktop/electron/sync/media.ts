import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { basename, join, parse } from "node:path";
import type { DatabaseSync } from "node:sqlite";

export const MEDIA_FLUSH_BATCH = 20;
export const MEDIA_PROGRESS_STEP = 64 * 1024;

export type MediaProgress = { hash: string; loaded: number; total: number };

export type MediaRow = {
  hash: string;
  noteId: string;
  accountId: string;
  mime: string;
  mediaType: string;
  width: number;
  height: number;
  durationSeconds: number;
  bytes: number;
  originalFilename: string;
  localPath: string | null;
  thumbPath: string | null;
  uploadState: string;
};

type StoredMediaRow = {
  hash: string;
  note_id: string;
  account_id: string;
  mime: string;
  media_type: string;
  width: number;
  height: number;
  duration_seconds: number;
  bytes: number;
  original_filename: string;
  local_path: string | null;
  thumb_path: string | null;
  upload_state: string;
};

export type MediaSyncOptions = {
  db: DatabaseSync;
  backendUrl: string;
  getAccessToken: () => Promise<string | null>;
  fetch?: typeof globalThis.fetch;
  onProgress?: (progress: MediaProgress) => void;
  logger?: Pick<Console, "error">;
};

export type MediaResolved = {
  state: "local" | "remote" | "missing";
  fullPath: string | null;
  thumbPath: string | null;
};

export class MediaRepository {
  readonly #db: DatabaseSync;
  readonly #media: MediaSync;
  readonly #mediaDirectory: string;
  readonly #downloadsDirectory: string;

  constructor(options: {
    db: DatabaseSync;
    media: MediaSync;
    dataDirectory: string;
    downloadsDirectory: string;
  }) {
    this.#db = options.db;
    this.#media = options.media;
    this.#mediaDirectory = join(options.dataDirectory, "media");
    this.#downloadsDirectory = options.downloadsDirectory;
  }

  store(input: {
    noteId: string;
    accountId: string;
    hash: string;
    mime: string;
    mediaType: string;
    width: number;
    height: number;
    durationSeconds: number;
    originalFilename: string;
    bytes: Uint8Array | number[];
    thumb: Uint8Array | number[];
  }): void {
    const bytes = new Uint8Array(input.bytes);
    const thumb = new Uint8Array(input.thumb);
    verifyMediaBytes(bytes, input.hash);
    const thumbDirectory = join(this.#mediaDirectory, "thumb");
    mkdirSync(thumbDirectory, { recursive: true });
    const fullPath = join(this.#mediaDirectory, input.hash);
    const thumbPath = join(thumbDirectory, `${input.hash}.jpg`);
    writeFileSync(fullPath, bytes);
    writeFileSync(thumbPath, thumb);
    this.#db
      .prepare(
        `INSERT INTO notebook_media
         (hash, note_id, account_id, mime, media_type, width, height, duration_seconds,
          bytes, original_filename, local_path, thumb_path, upload_state)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'pending')
         ON CONFLICT(hash) DO UPDATE SET note_id = excluded.note_id, account_id = excluded.account_id,
           mime = excluded.mime, media_type = excluded.media_type, width = excluded.width,
           height = excluded.height, duration_seconds = excluded.duration_seconds, bytes = excluded.bytes,
           original_filename = excluded.original_filename, local_path = excluded.local_path,
           thumb_path = excluded.thumb_path, upload_state = excluded.upload_state`,
      )
      .run(input.hash, input.noteId, input.accountId, input.mime, input.mediaType, input.width, input.height, input.durationSeconds, bytes.byteLength, input.originalFilename, fullPath, thumbPath);
  }

  resolve(hash: string): MediaResolved {
    const row = this.#find(hash);
    if (!row) return { state: "missing", fullPath: null, thumbPath: null };
    if (row.localPath && existsSync(row.localPath)) {
      return { state: "local", fullPath: row.localPath, thumbPath: row.thumbPath };
    }
    return { state: "remote", fullPath: null, thumbPath: null };
  }

  async ensure(noteId: string, hash: string): Promise<MediaResolved> {
    const resolved = this.resolve(hash);
    if (resolved.state === "local") return resolved;
    let download: Awaited<ReturnType<MediaSync["download"]>>;
    try {
      download = await this.#media.download(hash);
    } catch {
      return { state: "missing", fullPath: null, thumbPath: null };
    }
    verifyMediaBytes(download.full, hash);
    const thumbDirectory = join(this.#mediaDirectory, "thumb");
    mkdirSync(thumbDirectory, { recursive: true });
    const fullPath = join(this.#mediaDirectory, hash);
    writeFileSync(fullPath, download.full);
    const thumbPath = download.thumb ? join(thumbDirectory, `${hash}.jpg`) : null;
    if (download.thumb && thumbPath) writeFileSync(thumbPath, download.thumb);
    const current = this.#find(hash);
    if (current) {
      this.#db
        .prepare("UPDATE notebook_media SET local_path = ?, thumb_path = ?, upload_state = 'uploaded' WHERE hash = ?")
        .run(fullPath, thumbPath, hash);
    } else {
      this.#db
        .prepare(
          `INSERT INTO notebook_media
           (hash, note_id, account_id, mime, media_type, bytes, original_filename,
            local_path, thumb_path, upload_state)
           VALUES (?, ?, '', ?, ?, ?, '', ?, ?, 'uploaded')`,
        )
        .run(hash, noteId, download.mime, download.mime.startsWith("video/") ? "video" : "image", download.full.byteLength, fullPath, thumbPath);
    }
    return { state: "local", fullPath, thumbPath };
  }

  delete(hash: string): void {
    const row = this.#find(hash);
    for (const path of [row?.localPath, row?.thumbPath]) {
      if (!path) continue;
      try {
        unlinkSync(path);
      } catch {}
    }
    this.#db.prepare("DELETE FROM notebook_media WHERE hash = ?").run(hash);
  }

  save(hash: string, filename: string): string {
    const row = this.#find(hash);
    if (!row) throw new Error("media not found");
    if (!row.localPath) throw new Error("media has no local copy yet");
    if (!existsSync(row.localPath)) throw new Error("media file is not on disk");
    const extension = row.mime.split("/").at(-1) || "bin";
    const requestedStem = parse(basename(filename.trim())).name || "notebook-media";
    mkdirSync(this.#downloadsDirectory, { recursive: true });
    let target = join(this.#downloadsDirectory, `${requestedStem}.${extension}`);
    let suffix = 1;
    while (existsSync(target)) {
      target = join(this.#downloadsDirectory, `${requestedStem} (${suffix}).${extension}`);
      suffix += 1;
    }
    copyFileSync(row.localPath, target);
    return target;
  }

  #find(hash: string): MediaRow | null {
    const row = this.#db
      .prepare(
        `SELECT hash, note_id, account_id, mime, media_type, width, height, duration_seconds,
                bytes, original_filename, local_path, thumb_path, upload_state
         FROM notebook_media WHERE hash = ?`,
      )
      .get(hash) as StoredMediaRow | undefined;
    return row ? toMediaRow(row) : null;
  }
}

export class MediaSync {
  readonly #db: DatabaseSync;
  readonly #origin: string;
  readonly #getAccessToken: () => Promise<string | null>;
  readonly #fetch: typeof globalThis.fetch;
  readonly #onProgress: ((progress: MediaProgress) => void) | undefined;
  readonly #logger: Pick<Console, "error">;

  constructor(options: MediaSyncOptions) {
    this.#db = options.db;
    this.#origin = backendOrigin(options.backendUrl);
    this.#getAccessToken = options.getAccessToken;
    this.#fetch = options.fetch ?? globalThis.fetch;
    this.#onProgress = options.onProgress;
    this.#logger = options.logger ?? console;
  }

  async flush(accountId: string): Promise<number> {
    const rows = this.#db
      .prepare(
        `SELECT hash, note_id, account_id, mime, media_type, width, height,
                duration_seconds, bytes, original_filename, local_path, thumb_path, upload_state
         FROM notebook_media
         WHERE upload_state = 'pending' AND account_id = ?
         ORDER BY created_at ASC LIMIT ?`,
      )
      .all(accountId, MEDIA_FLUSH_BATCH) as StoredMediaRow[];
    let uploaded = 0;
    for (const stored of rows) {
      const row = toMediaRow(stored);
      if (!row.localPath || !existsSync(row.localPath)) {
        this.#logger.error(`media sync: ${row.hash} has no readable local path, skipping`);
        continue;
      }
      const filename = row.originalFilename || row.hash;
      try {
        await this.upload(row.hash, row.noteId, row.mime, filename, readFileSync(row.localPath));
        this.#db.prepare("UPDATE notebook_media SET upload_state = 'uploaded' WHERE hash = ?").run(row.hash);
        uploaded += 1;
      } catch (error) {
        this.#logger.error(`media sync: upload ${row.hash} failed:`, error);
      }
    }
    return uploaded;
  }

  async upload(hash: string, noteId: string, mime: string, filename: string, bytes: Uint8Array): Promise<void> {
    const token = await this.#accessToken();
    const form = new FormData();
    form.set("noteId", noteId);
    form.set("hash", hash);
    form.set("file", new Blob([new Uint8Array(bytes).buffer], { type: mime }), filename || hash);
    const response = await this.#fetch(`${this.#origin}/notebook/media/upload`, {
      method: "POST",
      headers: { authorization: `Bearer ${token}` },
      body: form,
    });
    if (!response.ok) throw new Error(`media upload failed (${response.status}): ${await response.text()}`);
  }

  async download(hash: string): Promise<{ full: Uint8Array; thumb: Uint8Array | null; mime: string }> {
    const token = await this.#accessToken();
    const headers = { authorization: `Bearer ${token}` };
    const response = await this.#fetch(`${this.#origin}/notebook/media/${hash}`, { headers });
    if (!response.ok) throw new Error(`media download failed (${response.status}) for ${hash}`);
    const mime = response.headers.get("content-type") ?? "application/octet-stream";
    const total = Number(response.headers.get("content-length") ?? 0) || 0;
    const full = await readResponse(response, (loaded) => this.#onProgress?.({ hash, loaded, total }), total);

    const thumbResponse = await this.#fetch(`${this.#origin}/notebook/media/${hash}/thumb`, { headers });
    const thumb = thumbResponse.ok ? new Uint8Array(await thumbResponse.arrayBuffer()) : null;
    return { full, thumb, mime };
  }

  async #accessToken(): Promise<string> {
    const token = await this.#getAccessToken();
    if (!token) throw new Error("Not signed in");
    return token;
  }
}

export function backendOrigin(backendUrl: string): string {
  const trimmed = backendUrl.replace(/\/+$/, "");
  return trimmed.endsWith("/graphql") ? trimmed.slice(0, -"/graphql".length) : trimmed;
}

export function verifyMediaBytes(bytes: Uint8Array, expectedHash: string): void {
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== expectedHash) throw new Error(`media hash mismatch: expected ${expectedHash}, got ${actual}`);
}

async function readResponse(
  response: Response,
  progress: (loaded: number) => void,
  total: number,
): Promise<Uint8Array> {
  if (!response.body) return new Uint8Array(await response.arrayBuffer());
  const reader = response.body.getReader();
  const chunks: Uint8Array[] = [];
  let loaded = 0;
  let lastEmit = 0;
  for (;;) {
    const { done, value } = await reader.read();
    if (done) break;
    chunks.push(value);
    loaded += value.byteLength;
    if (loaded - lastEmit >= MEDIA_PROGRESS_STEP || loaded === total) {
      lastEmit = loaded;
      progress(loaded);
    }
  }
  const bytes = new Uint8Array(loaded);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

function toMediaRow(row: StoredMediaRow): MediaRow {
  return {
    hash: row.hash,
    noteId: row.note_id,
    accountId: row.account_id,
    mime: row.mime,
    mediaType: row.media_type,
    width: row.width,
    height: row.height,
    durationSeconds: row.duration_seconds,
    bytes: row.bytes,
    originalFilename: row.original_filename,
    localPath: row.local_path,
    thumbPath: row.thumb_path,
    uploadState: row.upload_state,
  };
}
