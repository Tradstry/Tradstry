// Tracks in-flight media uploads by their temporary id (the `local-…` id the
// paste plugin assigns before the server responds). Lets a still-uploading
// image/video node abort its own upload when the user cancels it.

const controllers = new Map<string, AbortController>();

export function registerUpload(tempId: string, controller: AbortController) {
  controllers.set(tempId, controller);
}

export function unregisterUpload(tempId: string) {
  controllers.delete(tempId);
}

/** Abort the upload for `tempId` if one is still in flight. No-op otherwise. */
export function abortUpload(tempId: string) {
  controllers.get(tempId)?.abort();
  controllers.delete(tempId);
}
