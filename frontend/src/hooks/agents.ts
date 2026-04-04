"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as agentsService from "@/lib/service/agents";

const AGENTS_KEY = ["user-agents"] as const;

export function useUserAgents(accountId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: [...AGENTS_KEY, accountId],
    queryFn: () => agentsService.fetchUserAgents(fetcher, accountId!),
    enabled: isLoaded && isSignedIn && !!accountId,
  });
}

export function useDeleteUserAgent() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => agentsService.deleteUserAgent(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: AGENTS_KEY });
    },
  });
}
