"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import type { UserPrompt } from "@tradstry/app-ui/lib/service/prompts";
import * as promptsService from "@tradstry/app-ui/lib/service/prompts";
import {
  optimisticCreate,
  optimisticRemove,
  optimisticUpdate,
  tempId,
} from "./optimistic";

const PROMPTS_KEY = ["user-prompts"] as const;
const stamp = () => new Date().toISOString();

export function useUserPrompts() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: PROMPTS_KEY,
    queryFn: () => promptsService.fetchUserPrompts(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useCreateUserPrompt() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ name, content }: { name: string; content: string }) =>
      promptsService.createUserPrompt(fetcher, name, content),
    ...optimisticCreate<{ name: string; content: string }, UserPrompt>(
      queryClient,
      PROMPTS_KEY,
      ({ name, content }) => ({
        id: tempId(),
        name,
        content,
        createdAt: stamp(),
        updatedAt: stamp(),
      }),
    ),
  });
}

export function useUpdateUserPrompt() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; name?: string; content?: string };
  return useMutation({
    mutationFn: ({ id, name, content }: UpdateVars) =>
      promptsService.updateUserPrompt(fetcher, id, name, content),
    ...optimisticUpdate<UpdateVars, UserPrompt>(
      queryClient,
      PROMPTS_KEY,
      (vars) => vars.id,
      (entity, { name, content }) => ({
        ...entity,
        ...(name !== undefined ? { name } : {}),
        ...(content !== undefined ? { content } : {}),
      }),
    ),
  });
}

export function useDeleteUserPrompt() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => promptsService.deleteUserPrompt(fetcher, id),
    ...optimisticRemove<string>(queryClient, PROMPTS_KEY, (id) => id),
  });
}
