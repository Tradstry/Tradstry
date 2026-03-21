"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as aiService from "@/lib/service/ai";
import type {
  AiArtifactEnvelope,
  AiArtifactKind,
  AiArtifactRequest,
  AiJobHandle,
} from "@/lib/types/ai";

export function aiQueryKey(kind: AiArtifactKind, request: AiArtifactRequest | null) {
  return [
    "ai",
    kind,
    request?.accountId ?? null,
    request?.timeFilter.range ?? null,
    request?.timeFilter.startDate ?? null,
    request?.timeFilter.endDate ?? null,
  ] as const;
}

export function useAiArtifact(
  kind: AiArtifactKind,
  request: AiArtifactRequest | null,
) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<AiArtifactEnvelope | null>({
    queryKey: aiQueryKey(kind, request),
    queryFn: () => {
      if (!request) {
        throw new Error("AI artifact request is required");
      }

      if (kind === "insights") {
        return aiService.fetchAiInsights(fetcher, request);
      }

      if (kind === "report") {
        return aiService.fetchAiReport(fetcher, request);
      }

      return aiService.fetchMindsetSummary(fetcher, request);
    },
    enabled: isLoaded && isSignedIn && !!request?.accountId,
  });
}

export function useAiJobMutation(kind: AiArtifactKind) {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation<AiJobHandle, Error, AiArtifactRequest>({
    mutationFn: (request) => {
      if (kind === "insights") {
        return aiService.refreshAiInsights(fetcher, request);
      }

      if (kind === "report") {
        return aiService.generateAiReport(fetcher, request);
      }

      return aiService.refreshMindsetSummary(fetcher, request);
    },
    onSuccess: (_data, request) => {
      queryClient.invalidateQueries({ queryKey: aiQueryKey(kind, request) });
    },
  });
}
