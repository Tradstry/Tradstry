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
  query AIInsights($accountId: String!, $timeFilter: AiTimeFilterInput!) {
    aiInsights(accountId: $accountId, timeFilter: $timeFilter) {
      ${AI_ARTIFACT_FIELDS}
    }
  }
`;

const AI_REPORT_QUERY = `
  query AIReport($accountId: String!, $timeFilter: AiTimeFilterInput!) {
    aiReport(accountId: $accountId, timeFilter: $timeFilter) {
      ${AI_ARTIFACT_FIELDS}
    }
  }
`;

const MINDSET_QUERY = `
  query MindsetSummary($accountId: String!, $timeFilter: AiTimeFilterInput!) {
    mindsetSummary(accountId: $accountId, timeFilter: $timeFilter) {
      ${AI_ARTIFACT_FIELDS}
    }
  }
`;

const REFRESH_AI_INSIGHTS_MUTATION = `
  mutation RefreshAIInsights($accountId: String!, $timeFilter: AiTimeFilterInput!) {
    refreshAiInsights(accountId: $accountId, timeFilter: $timeFilter) {
      ${JOB_HANDLE_FIELDS}
    }
  }
`;

const GENERATE_AI_REPORT_MUTATION = `
  mutation GenerateAIReport($accountId: String!, $timeFilter: AiTimeFilterInput!) {
    generateAiReport(accountId: $accountId, timeFilter: $timeFilter) {
      ${JOB_HANDLE_FIELDS}
    }
  }
`;

const REFRESH_MINDSET_MUTATION = `
  mutation RefreshMindsetSummary($accountId: String!, $timeFilter: AiTimeFilterInput!) {
    refreshMindsetSummary(accountId: $accountId, timeFilter: $timeFilter) {
      ${JOB_HANDLE_FIELDS}
    }
  }
`;

export const AI_JOB_EVENTS_SUBSCRIPTION = `
  subscription AIJobEvents($jobId: String!) {
    aiJobEvents(jobId: $jobId) {
      jobId
      accountId
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
    { accountId: request.accountId, timeFilter: request.timeFilter },
  );
  return data.aiInsights;
}

export async function fetchAiReport(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiArtifactEnvelope | null> {
  const data = await fetcher<{ aiReport: AiArtifactEnvelope | null }>(
    AI_REPORT_QUERY,
    { accountId: request.accountId, timeFilter: request.timeFilter },
  );
  return data.aiReport;
}

export async function fetchMindsetSummary(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiArtifactEnvelope | null> {
  const data = await fetcher<{ mindsetSummary: AiArtifactEnvelope | null }>(
    MINDSET_QUERY,
    { accountId: request.accountId, timeFilter: request.timeFilter },
  );
  return data.mindsetSummary;
}

export async function refreshAiInsights(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiJobHandle> {
  const data = await fetcher<{ refreshAiInsights: AiJobHandle }>(
    REFRESH_AI_INSIGHTS_MUTATION,
    { accountId: request.accountId, timeFilter: request.timeFilter },
  );
  return data.refreshAiInsights;
}

export async function generateAiReport(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiJobHandle> {
  const data = await fetcher<{ generateAiReport: AiJobHandle }>(
    GENERATE_AI_REPORT_MUTATION,
    { accountId: request.accountId, timeFilter: request.timeFilter },
  );
  return data.generateAiReport;
}

export async function refreshMindsetSummary(
  fetcher: GraphQLFetcher,
  request: AiArtifactRequest,
): Promise<AiJobHandle> {
  const data = await fetcher<{ refreshMindsetSummary: AiJobHandle }>(
    REFRESH_MINDSET_MUTATION,
    { accountId: request.accountId, timeFilter: request.timeFilter },
  );
  return data.refreshMindsetSummary;
}
