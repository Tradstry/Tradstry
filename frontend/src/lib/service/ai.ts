import type { GraphQLFetcher } from "@/lib/client";
import type {
  AiArtifactEnvelope,
  AiArtifactRequest,
  AiJobHandle,
} from "@/lib/types/ai";

const SOURCE_CITATION_FIELDS = `
  sourceType
  sourceId
  title
  excerpt
`;

const AI_ARTIFACT_FIELDS = `
  artifactType
  insightBundle {
    overview
    cards {
      title
      summary
      category
      severity
      citations {
        ${SOURCE_CITATION_FIELDS}
      }
    }
    nextActions
  }
  report {
    title
    summary
    sections {
      heading
      body
      citations {
        ${SOURCE_CITATION_FIELDS}
      }
    }
    nextActions
  }
  mindsetSummary {
    overview
    signals {
      pattern
      evidence
      coaching
      citations {
        ${SOURCE_CITATION_FIELDS}
      }
    }
    routines
  }
`;

const JOB_HANDLE_FIELDS = `
  jobId
  status
`;

const AI_INSIGHTS_QUERY = `
  query AIInsights($workspaceId: String!, $timeFilter: AiTimeFilterInput!) {
    aiInsights(workspaceId: $workspaceId, timeFilter: $timeFilter) {
      ${AI_ARTIFACT_FIELDS}
    }
  }
`;

const AI_REPORT_QUERY = `
  query AIReport($workspaceId: String!, $timeFilter: AiTimeFilterInput!) {
    aiReport(workspaceId: $workspaceId, timeFilter: $timeFilter) {
      ${AI_ARTIFACT_FIELDS}
    }
  }
`;

const MINDSET_QUERY = `
  query MindsetSummary($workspaceId: String!, $timeFilter: AiTimeFilterInput!) {
    mindsetSummary(workspaceId: $workspaceId, timeFilter: $timeFilter) {
      ${AI_ARTIFACT_FIELDS}
    }
  }
`;

const REFRESH_AI_INSIGHTS_MUTATION = `
  mutation RefreshAIInsights($workspaceId: String!, $timeFilter: AiTimeFilterInput!) {
    refreshAiInsights(workspaceId: $workspaceId, timeFilter: $timeFilter) {
      ${JOB_HANDLE_FIELDS}
    }
  }
`;

const GENERATE_AI_REPORT_MUTATION = `
  mutation GenerateAIReport($workspaceId: String!, $timeFilter: AiTimeFilterInput!) {
    generateAiReport(workspaceId: $workspaceId, timeFilter: $timeFilter) {
      ${JOB_HANDLE_FIELDS}
    }
  }
`;

const REFRESH_MINDSET_MUTATION = `
  mutation RefreshMindsetSummary($workspaceId: String!, $timeFilter: AiTimeFilterInput!) {
    refreshMindsetSummary(workspaceId: $workspaceId, timeFilter: $timeFilter) {
      ${JOB_HANDLE_FIELDS}
    }
  }
`;

export const AI_JOB_EVENTS_SUBSCRIPTION = `
  subscription AIJobEvents($jobId: String!) {
    aiJobEvents(jobId: $jobId) {
      jobId
      workspaceId
      artifactType
      status
      message
      error
      artifact {
        ${AI_ARTIFACT_FIELDS}
      }
    }
  }
`;

export async function fetchAiInsights(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiArtifactEnvelope | null> {
  const data = await fetcher<{ aiInsights: AiArtifactEnvelope | null }>(
    AI_INSIGHTS_QUERY,
    { workspaceId: request.workspaceId, timeFilter: request.timeFilter },
  );
  return data.aiInsights;
}

export async function fetchAiReport(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiArtifactEnvelope | null> {
  const data = await fetcher<{ aiReport: AiArtifactEnvelope | null }>(
    AI_REPORT_QUERY,
    { workspaceId: request.workspaceId, timeFilter: request.timeFilter },
  );
  return data.aiReport;
}

export async function fetchMindsetSummary(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiArtifactEnvelope | null> {
  const data = await fetcher<{ mindsetSummary: AiArtifactEnvelope | null }>(
    MINDSET_QUERY,
    { workspaceId: request.workspaceId, timeFilter: request.timeFilter },
  );
  return data.mindsetSummary;
}

export async function refreshAiInsights(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiJobHandle> {
  const data = await fetcher<{ refreshAiInsights: AiJobHandle }>(
    REFRESH_AI_INSIGHTS_MUTATION,
    { workspaceId: request.workspaceId, timeFilter: request.timeFilter },
  );
  return data.refreshAiInsights;
}

export async function generateAiReport(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiJobHandle> {
  const data = await fetcher<{ generateAiReport: AiJobHandle }>(
    GENERATE_AI_REPORT_MUTATION,
    { workspaceId: request.workspaceId, timeFilter: request.timeFilter },
  );
  return data.generateAiReport;
}

export async function refreshMindsetSummary(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiJobHandle> {
  const data = await fetcher<{ refreshMindsetSummary: AiJobHandle }>(
    REFRESH_MINDSET_MUTATION,
    { workspaceId: request.workspaceId, timeFilter: request.timeFilter },
  );
  return data.refreshMindsetSummary;
}
