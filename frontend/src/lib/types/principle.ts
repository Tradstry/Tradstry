export interface PrincipleWithStats {
  id: string;
  userId: string;
  accountId: string;
  playbookId: string | null;
  evidenceNoteId: string | null;
  evidenceNoteTitle: string | null;
  title: string;
  theRule: string;
  why: string;
  intervention: string | null;
  priority: number;
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
  violationCount: number;
  violatedCumulativeProfit: number;
  violatedCumulativeRoi: number;
  violatedWinRate: number;
}

export interface CreatePrincipleInput {
  accountId: string;
  title: string;
  theRule: string;
  why: string;
  intervention?: string | null;
  playbookId?: string | null;
  evidenceNoteId?: string | null;
}

export interface UpdatePrincipleInput {
  title?: string;
  theRule?: string;
  why?: string;
  intervention?: string | null;
  clearIntervention?: boolean;
  playbookId?: string | null;
  clearPlaybook?: boolean;
  evidenceNoteId?: string | null;
  clearEvidenceNote?: boolean;
  isActive?: boolean;
}
