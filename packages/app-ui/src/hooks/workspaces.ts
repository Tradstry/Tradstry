"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as workspaceService from "@tradstry/app-ui/lib/service/workspaces";
import type {
  CreateWorkspaceInput,
  UpdateWorkspaceInput,
  Workspace,
} from "@tradstry/app-ui/lib/types/workspaces";
import { optimisticRemove, optimisticUpdate } from "./optimistic";

const WORKSPACES_KEY = ["workspaces"] as const;

export function useWorkspaces() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<Workspace[]>({
    queryKey: WORKSPACES_KEY,
    queryFn: () => workspaceService.fetchWorkspaces(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useWorkspace(id: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<Workspace | null>({
    queryKey: [...WORKSPACES_KEY, id],
    queryFn: () => {
      if (!id) throw new Error("workspace id is required");
      return workspaceService.fetchWorkspace(fetcher, id);
    },
    enabled: isLoaded && isSignedIn && !!id,
  });
}

export function useCreateWorkspace() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateWorkspaceInput) =>
      workspaceService.createWorkspace(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: WORKSPACES_KEY });
    },
  });
}

export function useUpdateWorkspace() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdateWorkspaceInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      workspaceService.updateWorkspace(fetcher, id, input),
    ...optimisticUpdate<UpdateVars, Workspace>(
      queryClient,
      WORKSPACES_KEY,
      (vars) => vars.id,
      (entity, { input }) => ({ ...entity, ...input }) as Workspace,
    ),
  });
}

export function useDeleteWorkspace() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => workspaceService.deleteWorkspace(fetcher, id),
    ...optimisticRemove<string>(queryClient, WORKSPACES_KEY, (id) => id),
  });
}
