"use client";

import {
  useCreateWorkspace,
  useDeleteWorkspace,
  useUpdateWorkspace,
  useWorkspaces as useWorkspacesQuery,
} from "@/hooks/workspaces";
import { useActiveWorkspaceStore } from "./store";
import type { Workspace } from "./types";

export function useWorkspaces(): Workspace[] {
  const { data } = useWorkspacesQuery();
  return data ?? [];
}

export function useWorkspacesLoading(): boolean {
  const { isLoading, isPending } = useWorkspacesQuery();
  return isLoading || isPending;
}

export function useWorkspacesError(): string | null {
  const { error } = useWorkspacesQuery();
  return error instanceof Error ? error.message : null;
}

export function useActiveWorkspace(): Workspace | null {
  const workspaces = useWorkspaces();
  const activeWorkspaceId = useActiveWorkspaceStore((s) => s.activeWorkspaceId);

  if (workspaces.length === 0) return null;

  const found = workspaces.find(
    (workspace) => workspace.id === activeWorkspaceId,
  );
  return found ?? workspaces[0] ?? null;
}

export function useWorkspaceActions() {
  const createMutation = useCreateWorkspace();
  const updateMutation = useUpdateWorkspace();
  const deleteMutation = useDeleteWorkspace();
  const store = useActiveWorkspaceStore();

  return {
    setActive: store.setActiveWorkspaceId,

    create: (data: {
      name: string;
      icon: string;
      currency: string;
      assetClass: Workspace["assetClass"];
      broker: string | null;
      riskProfile: string;
    }) => {
      createMutation.mutate(data, {
        onSuccess: (workspace) => {
          store.setActiveWorkspaceId(workspace.id);
        },
      });
    },

    update: (
      id: string,
      data: {
        name?: string;
        icon?: string;
        currency?: string;
        assetClass?: Workspace["assetClass"];
        broker?: string | null;
        riskProfile?: string;
      },
    ) => {
      updateMutation.mutate({ id, input: data });
    },

    delete: (id: string, workspaces: Workspace[]) => {
      const remaining = workspaces.filter((workspace) => workspace.id !== id);
      store.clearIfDeleted(id, remaining[0]?.id ?? null);
      deleteMutation.mutate(id);
    },
  };
}
