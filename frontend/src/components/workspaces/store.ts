import { create } from "zustand";
import { persist } from "zustand/middleware";

interface ActiveWorkspaceStore {
  activeWorkspaceId: string | null;
  setActiveWorkspaceId: (id: string) => void;
  clearIfDeleted: (deletedId: string, fallbackId: string | null) => void;
}

export const useActiveWorkspaceStore = create<ActiveWorkspaceStore>()(
  persist(
    (set, get) => ({
      activeWorkspaceId: null,

      setActiveWorkspaceId: (id) => {
        set({ activeWorkspaceId: id });
      },

      clearIfDeleted: (deletedId, fallbackId) => {
        if (get().activeWorkspaceId === deletedId) {
          set({ activeWorkspaceId: fallbackId });
        }
      },
    }),
    {
      name: "tradstry-active-workspace",
    },
  ),
);
