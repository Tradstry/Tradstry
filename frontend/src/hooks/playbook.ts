"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { capture, EVENTS } from "@/lib/analytics/events";
import { useGraphQL } from "@/lib/client";
import * as playbookService from "@/lib/service/playbook";
import type {
  CreatePlaybookInput,
  PlaybookWithStats,
  UpdatePlaybookInput,
} from "@/lib/types/playbook";
import { optimisticRemove, optimisticUpdate } from "./optimistic";

const PLAYBOOK_KEY = ["playbooks"] as const;

export function usePlaybooks() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<PlaybookWithStats[]>({
    queryKey: PLAYBOOK_KEY,
    queryFn: () => playbookService.fetchPlaybooks(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function usePlaybook(id: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<PlaybookWithStats | null>({
    queryKey: [...PLAYBOOK_KEY, id],
    queryFn: () => playbookService.fetchPlaybook(fetcher, id!),
    enabled: isLoaded && isSignedIn && !!id,
  });
}

export function useCreatePlaybook() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreatePlaybookInput) =>
      playbookService.createPlaybook(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PLAYBOOK_KEY });
      capture(EVENTS.playbookCreated, {});
    },
  });
}

export function useUpdatePlaybook() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdatePlaybookInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      playbookService.updatePlaybook(fetcher, id, input),
    ...optimisticUpdate<UpdateVars, PlaybookWithStats>(
      queryClient,
      PLAYBOOK_KEY,
      (vars) => vars.id,
      (entity, { input }) => ({ ...entity, ...input }) as PlaybookWithStats,
    ),
  });
}

export function useDeletePlaybook() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => playbookService.deletePlaybook(fetcher, id),
    ...optimisticRemove<string>(queryClient, PLAYBOOK_KEY, (id) => id),
  });
}
