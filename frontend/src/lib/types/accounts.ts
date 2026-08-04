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

export interface Account {
  id: string;
  userId: string;
  name: string;
  icon: string;
  currency: Currency;
  broker: string | null;
  riskProfile: RiskProfile;
  /** Account equity (positions + cash), synced from the brokerage. Null when unsynced. */
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

export interface CreateAccountInput {
  name: string;
  icon?: string;
  currency?: string;
  broker?: string | null;
  riskProfile?: string;
}

export interface UpdateAccountInput {
  name?: string;
  icon?: string;
  currency?: string;
  broker?: string | null;
  riskProfile?: string;
}
