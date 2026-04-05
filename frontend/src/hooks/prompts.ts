"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as promptsService from "@/lib/service/prompts";

const PROMPTS_KEY = ["user-prompts"] as const;

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
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PROMPTS_KEY });
    },
  });
}

export function useUpdateUserPrompt() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name, content }: { id: string; name?: string; content?: string }) =>
      promptsService.updateUserPrompt(fetcher, id, name, content),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PROMPTS_KEY });
    },
  });
}

export function useDeleteUserPrompt() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => promptsService.deleteUserPrompt(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: PROMPTS_KEY });
    },
  });
}
