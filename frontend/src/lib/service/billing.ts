import type { GraphQLFetcher } from "@/lib/client";
import type { BillingInfo, CheckoutInfo, PlanId } from "@/lib/types/billing";

const BILLING_QUERY = `
  query Billing {
    billing {
      plan
      status
      periodEnd
      cancelsAtPeriodEnd
      meters {
        ai { used limit }
        connections { used limit }
        data { used limit }
        media { used limit }
      }
    }
  }
`;

const CHECKOUT_INFO_QUERY = `
  query CheckoutInfo($plan: String!) {
    checkoutInfo(plan: $plan) {
      priceId
      paddleCustomerId
      userId
    }
  }
`;

const CREATE_BILLING_PORTAL_SESSION_MUTATION = `
  mutation CreateBillingPortalSession {
    createBillingPortalSession
  }
`;

export async function fetchBilling(
  fetcher: GraphQLFetcher,
): Promise<BillingInfo> {
  const data = await fetcher<{ billing: BillingInfo }>(BILLING_QUERY);
  return data.billing;
}

export async function fetchCheckoutInfo(
  fetcher: GraphQLFetcher,
  plan: PlanId,
): Promise<CheckoutInfo> {
  const data = await fetcher<{ checkoutInfo: CheckoutInfo }>(
    CHECKOUT_INFO_QUERY,
    { plan },
  );
  return data.checkoutInfo;
}

/** Null when the user has never subscribed — there is no portal to open yet. */
export async function createBillingPortalSession(
  fetcher: GraphQLFetcher,
): Promise<string | null> {
  const data = await fetcher<{ createBillingPortalSession: string | null }>(
    CREATE_BILLING_PORTAL_SESSION_MUTATION,
  );
  return data.createBillingPortalSession;
}
