export type PlanId = "free" | "pro" | "pro_plus";

/** A limit of `null` means unlimited. */
export interface Meter {
  used: number;
  limit: number | null;
}

export interface BillingMeters {
  ai: Meter;
  connections: Meter;
  data: Meter;
  media: Meter;
}

export interface BillingInfo {
  /** The plan in force now — during a grace or cancellation window this is not
   * the same as the tier the user subscribed to. */
  plan: PlanId;
  status: string | null;
  /** ISO timestamp: when the current quota window resets. */
  periodEnd: string;
  meters: BillingMeters;
  cancelsAtPeriodEnd: boolean;
}

export interface CheckoutInfo {
  priceId: string;
  paddleCustomerId: string | null;
  /** Sent as `custom_data.user_id` so the webhook can match the subscription. */
  userId: string;
}

/** The GraphQL error extension code the backend attaches when a plan limit is
 * hit, so callers can render an upgrade prompt instead of a generic failure. */
export const PLAN_LIMIT_CODE = "PLAN_LIMIT_REACHED";

export interface PlanLimitError {
  resource: string;
  limit: number;
  resetsAt: string | null;
  message: string;
}

/** Narrow an unknown thrown value to a plan-limit error.
 *
 * The GraphQL client attaches `extensions` to the thrown Error, which is where
 * the backend puts the code, the limit and the reset date. */
export function asPlanLimitError(error: unknown): PlanLimitError | null {
  if (!(error instanceof Error)) return null;

  const ext = (error as Error & { extensions?: Record<string, unknown> })
    .extensions;
  if (ext?.code !== PLAN_LIMIT_CODE) return null;

  return {
    resource: String(ext.resource ?? "this feature"),
    limit: Number(ext.limit ?? 0),
    resetsAt: (ext.resetsAt as string | undefined) ?? null,
    message: error.message,
  };
}
