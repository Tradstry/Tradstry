import { useEffect, useState } from "react";
import { NotebookIcon, PlusIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import { FolderList } from "./folder-list";
import { NoteList } from "./note-list";
import { NotebookEditor } from "./editor";
import {
  EMPTY_DOC,
  SEED_FOLDERS,
  SEED_NOTES,
  type Folder,
  type Note,
} from "./types";

export default function NotebookView() {
  const [folders, setFolders] = useState<Folder[]>(SEED_FOLDERS);
  const [notes, setNotes] = useState<Note[]>(SEED_NOTES);
  const [active, setActive] = useState<string>("all"); // "all" | "uncat" | folderId
  const [selectedId, setSelectedId] = useState<string | null>(
    SEED_NOTES[0]?.id ?? null,
  );

  const inActive = (n: Note) =>
    active === "all"
      ? true
      : active === "uncat"
        ? n.folderId == null
        : n.folderId === active;
  const filtered = notes.filter(inActive);

  // Keep a valid note selected as the folder/notes change.
  useEffect(() => {
    if (selectedId && filtered.some((n) => n.id === selectedId)) return;
    setSelectedId(filtered[0]?.id ?? null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [active, notes, selectedId]);

  const selectedNote = notes.find((n) => n.id === selectedId) ?? null;
  const now = () => new Date().toISOString();

  const createNote = () => {
    const folderId = active === "all" || active === "uncat" ? null : active;
    const note: Note = {
      id: crypto.randomUUID(),
      folderId,
      title: "Untitled",
      documentJson: EMPTY_DOC,
      tradeIds: [],
      sortOrder: 0,
      updatedAt: now(),
    };
    setNotes((prev) => [note, ...prev]);
    setSelectedId(note.id);
  };

  const deleteNote = (id: string) => {
    setNotes((prev) => prev.filter((n) => n.id !== id));
    if (selectedId === id) {
      const rest = filtered.filter((n) => n.id !== id);
      setSelectedId(rest[0]?.id ?? null);
    }
  };

  const updateContent = (id: string, json: string) =>
    setNotes((prev) =>
      prev.map((n) =>
        n.id === id
          ? n.documentJson === json
            ? n
            : { ...n, documentJson: json, updatedAt: now() }
          : n,
      ),
    );

  const updateTitle = (id: string, title: string) =>
    setNotes((prev) =>
      prev.map((n) => (n.id === id ? { ...n, title, updatedAt: now() } : n)),
    );

  const createFolder = (name: string) => {
    const folder: Folder = {
      id: crypto.randomUUID(),
      parentFolderId: null,
      name,
      sortOrder: folders.length,
    };
    setFolders((prev) => [...prev, folder]);
    setActive(folder.id);
  };
  const renameFolder = (id: string, name: string) =>
    setFolders((prev) => prev.map((f) => (f.id === id ? { ...f, name } : f)));
  const deleteFolder = (id: string) => {
    setFolders((prev) => prev.filter((f) => f.id !== id));
    setNotes((prev) =>
      prev.map((n) => (n.folderId === id ? { ...n, folderId: null } : n)),
    );
    if (active === id) setActive("all");
  };

  const listTitle =
    active === "all"
      ? "All notes"
      : active === "uncat"
        ? "Uncategorized"
        : (folders.find((f) => f.id === active)?.name ?? "Notes");

  return (
    <div className="flex h-full min-h-0 bg-zinc-50/70 dark:bg-zinc-950/60">
      <FolderList
        folders={folders}
        notes={notes}
        active={active}
        onSelect={setActive}
        onCreateFolder={createFolder}
        onRenameFolder={renameFolder}
        onDeleteFolder={deleteFolder}
      />
      <NoteList
        title={listTitle}
        notes={filtered}
        selectedId={selectedId}
        onSelect={setSelectedId}
        onCreateNote={createNote}
        onDeleteNote={deleteNote}
      />
      <div className="flex min-h-0 flex-1 flex-col bg-white dark:bg-zinc-900/40">
        {selectedNote ? (
          <NotebookEditor
            key={selectedNote.id}
            documentJson={selectedNote.documentJson}
            onChange={(json) => updateContent(selectedNote.id, json)}
            header={
              <input
                value={selectedNote.title}
                onChange={(e) => updateTitle(selectedNote.id, e.target.value)}
                placeholder="Untitled"
                className="mb-3 w-full bg-transparent text-2xl font-bold tracking-tight text-zinc-900 outline-none placeholder:text-zinc-300 dark:text-zinc-50 dark:placeholder:text-zinc-600"
              />
            }
          />
        ) : (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center">
            <NotebookIcon size={28} className="text-zinc-300 dark:text-zinc-600" />
            <p className="text-sm text-muted-foreground">
              Select a note, or create a new one.
            </p>
            <Button type="button" size="sm" onClick={createNote}>
              <PlusIcon size={14} />
              New note
            </Button>
          </div>
        )}
      </div>
    </div>
  );
}
