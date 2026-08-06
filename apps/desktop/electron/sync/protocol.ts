export type GraphqlClient = (query: string, variables: Record<string, unknown>) => Promise<unknown>;

export type OutboxRow = {
  id: number;
  name: string;
  args: string;
  hlc: string;
};

export type WireNote = {
  id: string;
  folderId: string | null;
  title: string;
  documentJson: string;
  sortOrder: number;
  tradeIds: string[];
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type WireFolder = {
  id: string;
  parentFolderId: string | null;
  name: string;
  sortOrder: number;
  isSystem: boolean;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type PullResult = {
  cookie: string | null;
  lastMutationId: number;
  notes: WireNote[];
  folders: WireFolder[];
};

export type WirePlaybook = {
  id: string;
  name: string;
  edgeName: string;
  entryRules: string;
  exitRules: string;
  positionSizingRules: string;
  additionalRules: string | null;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type PlaybookPullResult = {
  cookie: string | null;
  lastMutationId: number;
  playbooks: WirePlaybook[];
};

export type WireJournalEntry = {
  id: string;
  openDate: string;
  closeDate: string;
  entryPrice: number;
  exitPrice: number;
  positionSize: number;
  stopLoss: number | null;
  symbol: string;
  symbolName: string;
  status: string;
  totalPl: number;
  netRoi: number;
  duration: number;
  riskReward: number | null;
  tradeType: string;
  playbookId: string | null;
  notes: string | null;
  broke30MinRule: boolean | null;
  preTradeConviction: number | null;
  marketRegime: string | null;
  isPlannedPreMarket: boolean | null;
  revengeTrade: boolean | null;
  ruleAdherenceScore: number | null;
  tagIds: string[];
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type JournalPullResult = {
  cookie: string | null;
  lastMutationId: number;
  entries: WireJournalEntry[];
};

export type WirePrinciple = {
  id: string;
  accountId: string;
  playbookId: string | null;
  evidenceNoteId: string | null;
  title: string;
  theRule: string;
  why: string;
  intervention: string | null;
  priority: number;
  isActive: boolean;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type PrinciplePullResult = {
  cookie: string | null;
  lastMutationId: number;
  principles: WirePrinciple[];
};

export type WireTagCategory = {
  id: string;
  name: string;
  role: string | null;
  color: string | null;
  sortOrder: number;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type WireTag = {
  id: string;
  categoryId: string;
  name: string;
  color: string | null;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type TagsPullResult = {
  cookie: string | null;
  lastMutationId: number;
  categories: WireTagCategory[];
  tags: WireTag[];
};

export type WireRule = {
  id: string;
  accountId: string;
  accountBalance: number;
  accountRisk: number;
  maxStopLossPct: number;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type WirePlan = {
  id: string;
  symbol: string;
  positionType: string;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  totalShares: number;
  positionValue: number;
  status: string;
  tranchesJson: string;
  notes: string | null;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type WireHistory = {
  id: string;
  symbol: string;
  positionType: string;
  entryPrice: number;
  stopLoss: number;
  accountBalance: number;
  accountRisk: number;
  shares: number;
  positionValue: number;
  accountPct: number;
  stopLossPct: number;
  hlc: string;
  deletedAt: string | null;
  updatedAt: string;
};

export type CalculatorPullResult = {
  cookie: string | null;
  lastMutationId: number;
  rules: WireRule[];
  plans: WirePlan[];
  history: WireHistory[];
};

export type WireAccount = {
  id: string;
  name: string;
  broker: string | null;
  currency: string | null;
  icon: string | null;
  totalValue: number | null;
  riskProfile: string | null;
};

export type RemoteUpdate = { noteId: string; seq: number; update: string };

const PUSH = `mutation PushNotebook($input: NotebookPushInput!) {
  pushNotebook(input: $input) { lastMutationId }
}`;
const PULL = `query PullNotebook($cookie: String, $accountId: String!, $clientId: String!) {
  pullNotebook(cookie: $cookie, accountId: $accountId, clientId: $clientId) {
    cookie lastMutationId
    notes { id folderId title documentJson sortOrder tradeIds hlc deletedAt updatedAt }
    folders { id parentFolderId name sortOrder isSystem hlc deletedAt updatedAt }
  }
}`;
const PULL_PLAYBOOK = `query PullPlaybook($cookie: String, $clientId: String!) {
  pullPlaybook(cookie: $cookie, clientId: $clientId) {
    cookie lastMutationId
    playbooks { id name edgeName entryRules exitRules positionSizingRules additionalRules hlc deletedAt updatedAt }
  }
}`;
const PULL_JOURNAL = `query PullJournal($cookie: String, $accountId: String!, $clientId: String!) {
  pullJournal(cookie: $cookie, accountId: $accountId, clientId: $clientId) {
    cookie lastMutationId
    entries { id openDate closeDate entryPrice exitPrice positionSize stopLoss symbol symbolName status totalPl netRoi duration riskReward tradeType playbookId notes broke30MinRule preTradeConviction marketRegime isPlannedPreMarket revengeTrade ruleAdherenceScore tagIds hlc deletedAt updatedAt }
  }
}`;
const PULL_PRINCIPLE = `query PullPrinciple($cookie: String, $accountId: String!, $clientId: String!) {
  pullPrinciple(cookie: $cookie, accountId: $accountId, clientId: $clientId) {
    cookie lastMutationId
    principles { id accountId playbookId evidenceNoteId title theRule why intervention priority isActive hlc deletedAt updatedAt }
  }
}`;
const PULL_TAGS = `query PullTags($cookie: String, $clientId: String!) {
  pullTags(cookie: $cookie, clientId: $clientId) {
    cookie lastMutationId
    categories { id name role color sortOrder hlc deletedAt updatedAt }
    tags { id categoryId name color hlc deletedAt updatedAt }
  }
}`;
const PULL_CALCULATOR = `query PullCalculator($cookie: String, $clientId: String!) {
  pullCalculator(cookie: $cookie, clientId: $clientId) {
    cookie lastMutationId
    rules { id accountId accountBalance accountRisk maxStopLossPct hlc deletedAt updatedAt }
    plans { id symbol positionType entryPrice stopLoss accountBalance accountRisk totalShares positionValue status tranchesJson notes hlc deletedAt updatedAt }
    history { id symbol positionType entryPrice stopLoss accountBalance accountRisk shares positionValue accountPct stopLossPct hlc deletedAt updatedAt }
  }
}`;
const PULL_ACCOUNTS = `query PullAccounts { accounts { id name broker currency icon totalValue riskProfile } }`;
const PULL_UPDATES = `query NotebookAccountUpdatesSince($accountId: String!, $sinceSeq: Int!) {
  notebookAccountUpdatesSince(accountId: $accountId, sinceSeq: $sinceSeq) { noteId seq update }
}`;

function required<T>(value: T | null | undefined, field: string): T {
  if (value === null || value === undefined) throw new Error(`missing ${field} in response`);
  return value;
}

export class SyncProtocol {
  readonly #graphql: GraphqlClient;

  constructor(graphql: GraphqlClient) {
    this.#graphql = graphql;
  }

  async push(clientId: string, accountId: string, mutations: OutboxRow[]): Promise<number> {
    if (mutations.length === 0) return 0;
    const ordered = [...mutations].sort((left, right) => left.id - right.id);
    const data = (await this.#graphql(PUSH, {
      input: { clientId, accountId, mutations: ordered },
    })) as { pushNotebook?: { lastMutationId?: number } };
    return required(data.pushNotebook?.lastMutationId, "lastMutationId");
  }

  async pull(clientId: string, accountId: string, cookie: string | null): Promise<PullResult> {
    const data = (await this.#graphql(PULL, { cookie, accountId, clientId })) as { pullNotebook?: PullResult };
    return required(data.pullNotebook, "pullNotebook");
  }

  async pullPlaybook(clientId: string, cookie: string | null): Promise<PlaybookPullResult> {
    const data = (await this.#graphql(PULL_PLAYBOOK, { cookie, clientId })) as { pullPlaybook?: PlaybookPullResult };
    return required(data.pullPlaybook, "pullPlaybook");
  }

  async pullJournal(clientId: string, accountId: string, cookie: string | null): Promise<JournalPullResult> {
    const data = (await this.#graphql(PULL_JOURNAL, { cookie, accountId, clientId })) as { pullJournal?: JournalPullResult };
    return required(data.pullJournal, "pullJournal");
  }

  async pullPrinciple(clientId: string, accountId: string, cookie: string | null): Promise<PrinciplePullResult> {
    const data = (await this.#graphql(PULL_PRINCIPLE, { cookie, accountId, clientId })) as { pullPrinciple?: PrinciplePullResult };
    return required(data.pullPrinciple, "pullPrinciple");
  }

  async pullTags(clientId: string, cookie: string | null): Promise<TagsPullResult> {
    const data = (await this.#graphql(PULL_TAGS, { cookie, clientId })) as { pullTags?: TagsPullResult };
    return required(data.pullTags, "pullTags");
  }

  async pullCalculator(clientId: string, cookie: string | null): Promise<CalculatorPullResult> {
    const data = (await this.#graphql(PULL_CALCULATOR, { cookie, clientId })) as { pullCalculator?: CalculatorPullResult };
    return required(data.pullCalculator, "pullCalculator");
  }

  async pullAccounts(): Promise<WireAccount[]> {
    const data = (await this.#graphql(PULL_ACCOUNTS, {})) as { accounts?: WireAccount[] };
    return required(data.accounts, "accounts");
  }

  async pullUpdates(accountId: string, sinceSeq: number): Promise<RemoteUpdate[]> {
    const data = (await this.#graphql(PULL_UPDATES, {
      accountId,
      sinceSeq,
    })) as { notebookAccountUpdatesSince?: RemoteUpdate[] };
    return required(data.notebookAccountUpdatesSince, "notebookAccountUpdatesSince");
  }
}

export function createGraphqlClient(options: {
  endpoint: string;
  getAccessToken: () => Promise<string | null>;
  fetch?: typeof globalThis.fetch;
}): GraphqlClient {
  const fetcher = options.fetch ?? globalThis.fetch;
  return async (query: string, variables: Record<string, unknown>): Promise<unknown> => {
    const token = await options.getAccessToken();
    if (!token) throw new Error("Not signed in");
    const response = await fetcher(options.endpoint, {
      method: "POST",
      headers: { authorization: `Bearer ${token}`, "content-type": "application/json" },
      body: JSON.stringify({ query, variables }),
    });
    const payload = (await response.json()) as { data?: unknown; errors?: unknown };
    if (payload.errors != null) throw new Error(`GraphQL error: ${JSON.stringify(payload.errors)}`);
    if (!response.ok) throw new Error(`Backend returned ${response.status}`);
    return required(payload.data, "data");
  };
}
