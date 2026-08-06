"use client";

import { create } from "zustand";
import { useChatStore } from "@tradstry/app-ui/hooks/chat";

interface NotebookPanelStore {
  isOpen: boolean;
  setOpen: (open: boolean) => void;
  toggleOpen: () => void;
}

// Opening the notebook "Manage Notes" panel closes the chat panel so the two
// right-side rails are mutually exclusive (they share the right edge).
export const useNotebookPanelStore = create<NotebookPanelStore>((set) => ({
  isOpen: false,
  setOpen: (open) => {
    if (open) useChatStore.getState().setOpen(false);
    set({ isOpen: open });
  },
  toggleOpen: () =>
    set((s) => {
      const next = !s.isOpen;
      if (next) useChatStore.getState().setOpen(false);
      return { isOpen: next };
    }),
}));
