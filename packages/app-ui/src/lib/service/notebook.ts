import { type GraphQLFetcher, getBackendBaseUrl } from "@tradstry/app-ui/lib/client";
import type {
  CreateNotebookFolderInput,
  CreateNotebookNoteInput,
  MoveNotebookNodeInput,
  NotebookFolder,
  NotebookImage,
  NotebookNote,
  UpdateNotebookNoteInput,
} from "@tradstry/app-ui/lib/types/notebook";

const NOTEBOOK_NOTE_FIELDS = `
  id
  userId
  workspaceId
  title
  documentJson
  tradeIds
  images {
    id
    noteId
    userId
    workspaceId
    cloudinaryAssetId
    cloudinaryPublicId
    secureUrl
    contentHash
    width
    height
    format
    bytes
    originalFilename
    mediaType
    contentType
    durationSeconds
    createdAt
  }
  folderId
  sortOrder
  isStarred
  isPinned
  createdAt
  updatedAt
`;

const NOTEBOOK_FOLDER_FIELDS = `
  id
  userId
  workspaceId
  parentFolderId
  name
  sortOrder
  isSystem
  createdAt
  updatedAt
`;

const NOTEBOOK_NOTES_QUERY = `
  query NotebookNotes($workspaceId: String) {
    notebookNotes(workspaceId: $workspaceId) {
      ${NOTEBOOK_NOTE_FIELDS}
    }
  }
`;

const NOTEBOOK_NOTE_QUERY = `
  query NotebookNote($id: String!) {
    notebookNote(id: $id) {
      ${NOTEBOOK_NOTE_FIELDS}
    }
  }
`;

const CREATE_NOTEBOOK_NOTE_MUTATION = `
  mutation CreateNotebookNote($input: CreateNotebookNoteInput!) {
    createNotebookNote(input: $input) {
      ${NOTEBOOK_NOTE_FIELDS}
    }
  }
`;

const UPDATE_NOTEBOOK_NOTE_MUTATION = `
  mutation UpdateNotebookNote($id: String!, $input: UpdateNotebookNoteInput!) {
    updateNotebookNote(id: $id, input: $input) {
      ${NOTEBOOK_NOTE_FIELDS}
    }
  }
`;

const DELETE_NOTEBOOK_NOTE_MUTATION = `
  mutation DeleteNotebookNote($id: String!) {
    deleteNotebookNote(id: $id)
  }
`;

const SET_NOTEBOOK_NOTE_FLAGS_MUTATION = `
  mutation SetNotebookNoteFlags($id: String!, $isStarred: Boolean, $isPinned: Boolean) {
    setNotebookNoteFlags(id: $id, isStarred: $isStarred, isPinned: $isPinned) {
      ${NOTEBOOK_NOTE_FIELDS}
    }
  }
`;

const NOTEBOOK_FOLDERS_QUERY = `
  query NotebookFolders($workspaceId: String!) {
    notebookFolders(workspaceId: $workspaceId) {
      ${NOTEBOOK_FOLDER_FIELDS}
    }
  }
`;

const CREATE_NOTEBOOK_FOLDER_MUTATION = `
  mutation CreateNotebookFolder($input: CreateNotebookFolderInput!) {
    createNotebookFolder(input: $input) {
      ${NOTEBOOK_FOLDER_FIELDS}
    }
  }
`;

const RENAME_NOTEBOOK_FOLDER_MUTATION = `
  mutation RenameNotebookFolder($id: String!, $name: String!) {
    renameNotebookFolder(id: $id, name: $name) {
      ${NOTEBOOK_FOLDER_FIELDS}
    }
  }
`;

const DELETE_NOTEBOOK_FOLDER_MUTATION = `
  mutation DeleteNotebookFolder($id: String!) {
    deleteNotebookFolder(id: $id)
  }
`;

const MOVE_NOTEBOOK_NODE_MUTATION = `
  mutation MoveNotebookNode($input: MoveNotebookNodeInput!) {
    moveNotebookNode(input: $input)
  }
`;

type TokenProvider = () => Promise<string | null>;

export async function fetchNotebookNotes(
  fetcher: GraphQLFetcher,
  workspaceId?: string | null,
): Promise<NotebookNote[]> {
  const data = await fetcher<{ notebookNotes: NotebookNote[] }>(
    NOTEBOOK_NOTES_QUERY,
    {
      workspaceId: workspaceId ?? null,
    },
  );
  return data.notebookNotes;
}

export async function fetchNotebookNote(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<NotebookNote | null> {
  const data = await fetcher<{ notebookNote: NotebookNote | null }>(
    NOTEBOOK_NOTE_QUERY,
    { id },
  );
  return data.notebookNote;
}

export async function createNotebookNote(
  fetcher: GraphQLFetcher,
  input: CreateNotebookNoteInput,
): Promise<NotebookNote> {
  const data = await fetcher<{ createNotebookNote: NotebookNote }>(
    CREATE_NOTEBOOK_NOTE_MUTATION,
    { input },
  );
  return data.createNotebookNote;
}

export async function updateNotebookNote(
  fetcher: GraphQLFetcher,
  id: string,
  input: UpdateNotebookNoteInput,
): Promise<NotebookNote> {
  const data = await fetcher<{ updateNotebookNote: NotebookNote }>(
    UPDATE_NOTEBOOK_NOTE_MUTATION,
    { id, input },
  );
  return data.updateNotebookNote;
}

export async function deleteNotebookNote(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteNotebookNote: boolean }>(
    DELETE_NOTEBOOK_NOTE_MUTATION,
    { id },
  );
  return data.deleteNotebookNote;
}

export async function setNotebookNoteFlags(
  fetcher: GraphQLFetcher,
  id: string,
  flags: { isStarred?: boolean; isPinned?: boolean },
): Promise<NotebookNote> {
  const data = await fetcher<{ setNotebookNoteFlags: NotebookNote }>(
    SET_NOTEBOOK_NOTE_FLAGS_MUTATION,
    { id, isStarred: flags.isStarred, isPinned: flags.isPinned },
  );
  return data.setNotebookNoteFlags;
}

export async function fetchNotebookFolders(
  fetcher: GraphQLFetcher,
  workspaceId: string,
): Promise<NotebookFolder[]> {
  const data = await fetcher<{ notebookFolders: NotebookFolder[] }>(
    NOTEBOOK_FOLDERS_QUERY,
    { workspaceId },
  );
  return data.notebookFolders;
}

export async function createNotebookFolder(
  fetcher: GraphQLFetcher,
  input: CreateNotebookFolderInput,
): Promise<NotebookFolder> {
  const data = await fetcher<{ createNotebookFolder: NotebookFolder }>(
    CREATE_NOTEBOOK_FOLDER_MUTATION,
    { input },
  );
  return data.createNotebookFolder;
}

export async function renameNotebookFolder(
  fetcher: GraphQLFetcher,
  id: string,
  name: string,
): Promise<NotebookFolder> {
  const data = await fetcher<{ renameNotebookFolder: NotebookFolder }>(
    RENAME_NOTEBOOK_FOLDER_MUTATION,
    { id, name },
  );
  return data.renameNotebookFolder;
}

export async function deleteNotebookFolder(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteNotebookFolder: boolean }>(
    DELETE_NOTEBOOK_FOLDER_MUTATION,
    { id },
  );
  return data.deleteNotebookFolder;
}

export async function moveNotebookNode(
  fetcher: GraphQLFetcher,
  input: MoveNotebookNodeInput,
): Promise<boolean> {
  const data = await fetcher<{ moveNotebookNode: boolean }>(
    MOVE_NOTEBOOK_NODE_MUTATION,
    { input },
  );
  return data.moveNotebookNode;
}

/** Reported during upload so callers can render progress. */
export interface UploadProgress {
  /** Bytes sent so far. */
  loaded: number;
  /** Total bytes to send. */
  total: number;
  /** 0–100. */
  percent: number;
}

function mapNotebookImage(img: Record<string, unknown>): NotebookImage {
  return {
    id: img.id,
    noteId: img.note_id,
    userId: img.user_id,
    workspaceId: img.workspace_id,
    cloudinaryAssetId: img.cloudinary_asset_id,
    cloudinaryPublicId: img.cloudinary_public_id,
    secureUrl: img.secure_url,
    contentHash: img.content_hash,
    width: img.width,
    height: img.height,
    format: img.format,
    bytes: img.bytes,
    originalFilename: img.original_filename,
    mediaType: img.media_type,
    contentType: img.content_type,
    durationSeconds: img.duration_seconds,
    createdAt: img.created_at,
  } as NotebookImage;
}

export async function uploadNotebookMedia(
  getToken: TokenProvider,
  noteId: string,
  hash: string,
  file: File,
  onProgress?: (progress: UploadProgress) => void,
  signal?: AbortSignal,
): Promise<NotebookImage> {
  if (signal?.aborted) {
    return Promise.reject(new Error("Upload aborted"));
  }

  const token = await getToken();
  const formData = new FormData();
  formData.set("noteId", noteId);
  formData.set("hash", hash);
  formData.set("file", file);

  // XMLHttpRequest (not fetch) so we can report upload progress via
  // upload.onprogress — fetch has no upload-progress API.
  return new Promise<NotebookImage>((resolve, reject) => {
    const xhr = new XMLHttpRequest();
    xhr.open("POST", `${getBackendBaseUrl()}/notebook/media/upload`);
    if (token) {
      xhr.setRequestHeader("Authorization", `Bearer ${token}`);
    }

    // Cancellation: aborting the signal aborts the request; xhr.onabort below
    // rejects the promise so callers can clean up.
    if (signal) {
      signal.addEventListener("abort", () => xhr.abort(), { once: true });
    }

    xhr.upload.onprogress = (event) => {
      if (!event.lengthComputable || !onProgress) {
        return;
      }
      onProgress({
        loaded: event.loaded,
        total: event.total,
        percent: Math.min(100, Math.round((event.loaded / event.total) * 100)),
      });
    };

    xhr.onload = () => {
      if (xhr.status >= 200 && xhr.status < 300) {
        try {
          const data = JSON.parse(xhr.responseText) as {
            image: Record<string, unknown>;
          };
          resolve(mapNotebookImage(data.image));
        } catch (error) {
          reject(error instanceof Error ? error : new Error(String(error)));
        }
      } else {
        reject(new Error(xhr.responseText || "Notebook media upload failed"));
      }
    };

    xhr.onerror = () => reject(new Error("Network error during upload"));
    xhr.onabort = () => reject(new Error("Upload aborted"));

    xhr.send(formData);
  });
}

export async function deleteNotebookImage(
  getToken: TokenProvider,
  hash: string,
  noteId: string,
): Promise<void> {
  const token = await getToken();

  const response = await fetch(
    `${getBackendBaseUrl()}/notebook/media/${hash}?noteId=${encodeURIComponent(noteId)}`,
    {
      method: "DELETE",
      headers: token
        ? {
            Authorization: `Bearer ${token}`,
          }
        : undefined,
    },
  );

  if (!response.ok) {
    const message = await response.text();
    throw new Error(message || "Notebook image delete failed");
  }
}
