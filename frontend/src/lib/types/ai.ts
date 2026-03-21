import type { AnalyticsTimeFilterInput } from "./analytics";

export type AiArtifactKind = "insights" | "report" | "mindset";

export interface SourceCitation {
  sourceType: string;
  sourceId: string;
  title: string;
  excerpt: string;
}

export interface InsightCard {
  title: string;
  summary: string;
  category: string;
  severity: string;
  citations: SourceCitation[];
}

export interface InsightBundle {
  overview: string;
  cards: InsightCard[];
  nextActions: string[];
}

export interface ReportSection {
  heading: string;
  body: string;
  citations: SourceCitation[];
}

export interface ReportArtifact {
  title: string;
  summary: string;
  sections: ReportSection[];
  nextActions: string[];
}

export interface MindsetSignal {
  pattern: string;
  evidence: string;
  coaching: string;
  citations: SourceCitation[];
}

export interface MindsetSummary {
  overview: string;
  signals: MindsetSignal[];
  routines: string[];
}

export interface AiArtifactEnvelope {
  artifactType: string;
  insightBundle: InsightBundle | null;
  report: ReportArtifact | null;
  mindsetSummary: MindsetSummary | null;
}

export interface AiJobHandle {
  jobId: string;
  status: string;
}

export interface AiJobEvent {
  jobId: string;
  accountId: string;
  artifactType: string | null;
  status: string;
  message: string | null;
  artifact: AiArtifactEnvelope | null;
  error: string | null;
}

export interface AiArtifactRequest {
  accountId: string;
  timeFilter: AnalyticsTimeFilterInput;
}
