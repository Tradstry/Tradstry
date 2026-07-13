import { useCallback, useEffect, useRef, useState } from "react";
import {
  CaretDownIcon,
  NotebookIcon,
  PlusIcon,
} from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import { cn } from "@/lib/utils";
import { FolderList } from "./folder-list";
import { NoteList } from "./note-list";
import { NotebookEditor } from "./editor";
import { type Folder, type Note } from "./types";
import {
  accounts,
  createFolder as createFolderCmd,
  createNote as createNoteCmd,
  deleteFolder as deleteFolderCmd,
  deleteNote as deleteNoteCmd,
  moveNote as moveNoteCmd,
  notebookFolders,
  notebookNotes,
  renameFolder as renameFolderCmd,
  type Account,
} from "../../../backend";

const COLLAPSED_KEY = "notebook:folders-collapsed";

function AccountSelect({
  accounts,
  value,
  onChange,
}: {
  accounts: Account[];
  value: string;
  onChange: (id: string) => void;
}) {
  const current = accounts.find((a) => a.id === value);
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <button
          type="button"
          className="flex h-8 cursor-pointer items-center gap-1.5 rounded-lg border border-zinc-200 bg-white px-3 text-sm font-medium text-zinc-700 outline-none transition duration-150 hover:bg-zinc-50 focus-visible:outline-2 focus-visible:outline-blue-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-200 dark:hover:bg-zinc-800"
        >
          {current?.name ?? "Account"}
          <CaretDownIcon size={13} className="text-zinc-400 dark:text-zinc-500" />
        </button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end" className="w-52">
        <DropdownMenuRadioGroup value={value} onValueChange={onChange}>
          {accounts.map((a) => (
            <DropdownMenuRadioItem key={a.id} value={a.id}>
              {a.name}
            </DropdownMenuRadioItem>
          ))}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

export default function NotebookView() {
  const [accountList, setAccountList] = useState<Account[]>([]);
  // undefined = resolving, null = no account, string = active account id
  const [accountId, setAccountId] = useState<string | null | undefined>(
    undefined,
  );
  const [folders, setFolders] = useState<Folder[]>([]);
  const [notes, setNotes] = useState<Note[]>([]);
  const [collapsed, setCollapsed] = useState(
    () => localStorage.getItem(COLLAPSED_KEY) === "true",
  );
  const [active, setActive] = useState<string>("all"); // "all" | "uncat" | folderId
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const accountIdRef = useRef<string | null>(null);
  useEffect(() => {
    accountIdRef.current = accountId ?? null;
  }, [accountId]);

  // The body is a CRDT: the editor writes update blobs straight to the local store,
  // so there is no document save to debounce or flush here. Only the cached title
  // and preview need refreshing once the editor rewrites them.
  const refreshNotes = useCallback(() => {
    const acct = accountIdRef.current;
    if (!acct) return;
    notebookNotes(acct)
      .then((fresh) => {
        if (accountIdRef.current === acct) setNotes(fresh);
      })
      .catch(() => {});
  }, []);

  useEffect(() => {
    accounts()
      .then((accs) => {
        setAccountList(accs);
        setAccountId(accs[0]?.id ?? null);
      })
      .catch(() => setAccountId(null));
  }, []);

  useEffect(() => {
    if (!accountId) {
      setNotes([]);
      setFolders([]);
      return;
    }
    let cancelled = false;
    Promise.all([notebookNotes(accountId), notebookFolders(accountId)])
      .then(([n, f]) => {
        if (cancelled) return;
        setNotes(n);
        setFolders(f);
      })
      .catch(() => {
        if (cancelled) return;
        setNotes([]);
        setFolders([]);
      });
    return () => {
      cancelled = true;
    };
  }, [accountId]);

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

  const toggleSidebar = useCallback(
    () =>
      setCollapsed((prev) => {
        localStorage.setItem(COLLAPSED_KEY, String(!prev));
        return !prev;
      }),
    [],
  );

  useEffect(() => {
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.metaKey && e.key === "\\") {
        e.preventDefault();
        toggleSidebar();
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [toggleSidebar]);

  const selectedNote = notes.find((n) => n.id === selectedId) ?? null;

  const selectNote = (id: string) => {
    setSelectedId(id);
  };

  const changeAccount = (id: string) => {
    if (id === accountId) return;
    setActive("all");
    setSelectedId(null);
    setAccountId(id);
  };

  const createNote = async () => {
    if (!accountId) return;
    const folderId = active === "all" || active === "uncat" ? null : active;
    const id = await createNoteCmd(accountId, folderId);
    setNotes(await notebookNotes(accountId));
    setSelectedId(id);
  };

  const deleteNote = async (id: string) => {
    if (!accountId) return;
    await deleteNoteCmd(id);
    setNotes(await notebookNotes(accountId));
  };

  const moveNote = async (noteId: string, folderId: string | null) => {
    if (!accountId) return;
    const target = notes.find((n) => n.id === noteId);
    if (!target || target.folderId === folderId) return;
    // Optimistic: reparent locally so the row jumps immediately, then reconcile.
    setNotes((prev) =>
      prev.map((n) => (n.id === noteId ? { ...n, folderId } : n)),
    );
    try {
      await moveNoteCmd(noteId, folderId, 0);
      if (accountIdRef.current === accountId) {
        setNotes(await notebookNotes(accountId));
      }
    } catch {
      if (accountIdRef.current === accountId) {
        setNotes(await notebookNotes(accountId));
      }
    }
  };

  const createFolder = async (name: string) => {
    if (!accountId) return;
    const id = await createFolderCmd(accountId, name);
    setFolders(await notebookFolders(accountId));
    setActive(id);
  };
  const renameFolder = async (id: string, name: string) => {
    if (!accountId) return;
    await renameFolderCmd(id, name);
    setFolders(await notebookFolders(accountId));
  };
  const deleteFolder = async (id: string) => {
    if (!accountId) return;
    await deleteFolderCmd(id);
    const [f, n] = await Promise.all([
      notebookFolders(accountId),
      notebookNotes(accountId),
    ]);
    setFolders(f);
    setNotes(n);
    if (active === id) setActive("all");
  };

  const listTitle =
    active === "all"
      ? "All notes"
      : active === "uncat"
        ? "Uncategorized"
        : (folders.find((f) => f.id === active)?.name ?? "Notes");

  if (accountId === undefined) {
    return (
      <p className="p-6 text-sm text-zinc-400 dark:text-zinc-600">Loading…</p>
    );
  }

  if (accountId === null) {
    return (
      <p className="p-6 text-sm text-zinc-500 dark:text-zinc-400">
        No account yet — connect a brokerage to start a notebook.
      </p>
    );
  }

  return (
    <div className="flex h-full min-h-0 bg-zinc-50/70 dark:bg-zinc-950/60">
      <div
        inert={collapsed}
        className={cn(
          "h-full shrink-0 overflow-hidden transition-[width] duration-200 ease-out motion-reduce:transition-none",
          collapsed ? "w-0" : "w-52",
        )}
      >
        <FolderList
          folders={folders}
          notes={notes}
          active={active}
          onSelect={setActive}
          onCreateFolder={createFolder}
          onRenameFolder={renameFolder}
          onDeleteFolder={deleteFolder}
          onMoveNote={moveNote}
        />
      </div>
      <NoteList
        title={listTitle}
        notes={filtered}
        selectedId={selectedId}
        onSelect={selectNote}
        onCreateNote={createNote}
        onDeleteNote={deleteNote}
        sidebarCollapsed={collapsed}
        onToggleSidebar={toggleSidebar}
      />
      <div className="flex min-h-0 flex-1 flex-col bg-white dark:bg-zinc-900/40">
        <div className="flex h-13 items-center justify-end gap-2 border-b border-zinc-200/70 px-4 dark:border-zinc-800/70">
          <AccountSelect
            accounts={accountList}
            value={accountId}
            onChange={changeAccount}
          />
        </div>
        {selectedNote ? (
          <NotebookEditor
            key={selectedNote.id}
            noteId={selectedNote.id}
            accountId={accountId}
            onBodyCached={refreshNotes}
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
