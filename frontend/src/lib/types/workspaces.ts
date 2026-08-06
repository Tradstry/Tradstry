export const CURRENCIES = [
  "USD",
  "EUR",
  "GBP",
  "JPY",
  "CAD",
  "AUD",
  "CHF",
] as const;

export type Currency = (typeof CURRENCIES)[number];

export type RiskProfile = "conservative" | "moderate" | "aggressive";
export type AssetClass =
  | "futures"
  | "options"
  | "stocks"
  | "forex"
  | "crypto"
  | "mixed"
  | "other";

export interface Workspace {
  id: string;
  userId: string;
  name: string;
  icon: string;
  currency: Currency;
  assetClass: AssetClass;
  broker: string | null;
  riskProfile: RiskProfile;
  /** Workspace equity (positions + cash), synced from the brokerage. Null when unsynced. */
  totalValue: number | null;
  totalValueCurrency: string | null;
  createdAt: string;
  updatedAt: string;
  snaptradeUserId: string | null;
  snaptradeConnectionId: string | null;
  snaptradeAccountId: string | null;
  snaptradeConnectionDisabled: boolean;
  snaptradeConnectionDisabledAt: string | null;
}

export interface CreateWorkspaceInput {
  name: string;
  icon?: string;
  currency?: string;
  assetClass?: AssetClass;
  broker?: string | null;
  riskProfile?: string;
}

export interface UpdateWorkspaceInput {
  name?: string;
  icon?: string;
  currency?: string;
  assetClass?: AssetClass;
  broker?: string | null;
  riskProfile?: string;
}
