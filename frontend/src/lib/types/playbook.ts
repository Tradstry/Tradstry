export interface Playbook {
  id: string;
  userId: string;
  workspaceId: string;
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface PlaybookWithStats extends Playbook {
  winRate: number;
  cumulativeProfit: number;
  averageGain: number;
  averageLoss: number;
  tradeCount: number;
}

export interface CreatePlaybookInput {
  workspaceId: string;
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules?: string | null;
}

export interface UpdatePlaybookInput {
  name?: string;
  edgeName?: string;
  entryRules?: string;
  exitRules?: string;
  positionSizingRules?: string;
  additionalRules?: string | null;
  clearAdditionalRules?: boolean;
}
