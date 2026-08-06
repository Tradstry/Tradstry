"use client";

import { useAuth } from "@tradstry/app-ui/platform";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@tradstry/app-ui/lib/client";
import * as agentsService from "@tradstry/app-ui/lib/service/agents";

const AGENTS_KEY = ["user-agents"] as const;

export function useUserAgents(workspaceId: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery({
    queryKey: [...AGENTS_KEY, workspaceId],
    queryFn: () => agentsService.fetchUserAgents(fetcher, workspaceId!),
    enabled: isLoaded && isSignedIn && !!workspaceId,
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
