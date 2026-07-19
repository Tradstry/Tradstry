"use client";

import { fetchCheckoutInfo } from "@/lib/service/billing";
import type { PlanId } from "@/lib/types/billing";

/** Paddle.js is loaded on demand — it's only needed when someone upgrades, and
 * pulling it on every page load would cost every user for a rare action. */
const PADDLE_JS = "https://cdn.paddle.com/paddle/v2/paddle.js";

interface PaddleGlobal {
  Environment: { set: (env: string) => void };
  Initialize: (options: {
    token: string;
    eventCallback?: (event: { name?: string }) => void;
  }) => void;
  Checkout: {
    open: (options: {
      items: Array<{ priceId: string; quantity: number }>;
      customer?: { id: string };
      customData?: Record<string, string>;
      settings?: { displayMode?: string; theme?: string };
    }) => void;
  };
}

declare global {
  interface Window {
    Paddle?: PaddleGlobal;
  }
}

let loading: Promise<PaddleGlobal> | null = null;

function loadPaddle(): Promise<PaddleGlobal> {
  if (window.Paddle) return Promise.resolve(window.Paddle);
  if (loading) return loading;

  loading = new Promise<PaddleGlobal>((resolve, reject) => {
    const script = document.createElement("script");
    script.src = PADDLE_JS;
    script.async = true;
    script.onload = () => {
      if (window.Paddle) resolve(window.Paddle);
      else reject(new Error("Paddle.js loaded but did not initialise"));
    };
    script.onerror = () => {
      loading = null;
      reject(new Error("Could not load the payment provider."));
    };
    document.head.appendChild(script);
  });

  return loading;
}

let initialised = false;

function initPaddle(paddle: PaddleGlobal, onComplete: () => void) {
  if (initialised) return;

  const token = process.env.NEXT_PUBLIC_PADDLE_CLIENT_TOKEN;
  if (!token) throw new Error("Checkout is not configured.");

  // Sandbox and live are separate Paddle accounts; the token only works
  // against its own environment.
  if (process.env.NEXT_PUBLIC_PADDLE_ENV === "sandbox") {
    paddle.Environment.set("sandbox");
  }

  paddle.Initialize({
    token,
    eventCallback: (event) => {
      if (event.name === "checkout.completed") onComplete();
    },
  });
  initialised = true;
}

/**
 * Open the Paddle overlay checkout for a tier.
 *
 * `customData.user_id` is the important part: it is what the webhook matches
 * the subscription back to a Tradstry user with.
 */
export async function openCheckout(
  fetcher: Parameters<typeof fetchCheckoutInfo>[0],
  plan: PlanId,
  onComplete: () => void,
): Promise<void> {
  const [paddle, info] = await Promise.all([
    loadPaddle(),
    fetchCheckoutInfo(fetcher, plan),
  ]);
  initPaddle(paddle, onComplete);

  paddle.Checkout.open({
    items: [{ priceId: info.priceId, quantity: 1 }],
    ...(info.paddleCustomerId
      ? { customer: { id: info.paddleCustomerId } }
      : {}),
    customData: { user_id: info.userId },
    settings: { displayMode: "overlay" },
  });
}
