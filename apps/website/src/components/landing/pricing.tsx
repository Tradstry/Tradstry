"use client";

import { SignUpButton } from "@clerk/nextjs";
import { Tick02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { PLAN_INCLUDES, PLANS } from "@/components/landing/content";
import { Reveal } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Pending,
  Section,
} from "@/components/landing/primitives";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { capture, EVENTS } from "@/lib/analytics/events";
import { cn } from "@tradstry/app-ui/lib/utils";

export function Pricing() {
  const [cadence, setCadence] = React.useState<"monthly" | "annual">("annual");
  const plan = PLANS.find((p) => p.id === cadence) ?? PLANS[0];

  const sectionRef = React.useRef<HTMLDivElement | null>(null);
  const fired = React.useRef(false);

  React.useEffect(() => {
    const node = sectionRef.current;
    if (!node) {
      return;
    }
    const observer = new IntersectionObserver((entries) => {
      if (entries[0]?.isIntersecting && !fired.current) {
        fired.current = true;
        capture(EVENTS.pricingViewed, {});
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  return (
    <div ref={sectionRef}>
      <Section id="pricing">
        <Reveal className="max-w-2xl">
          <Eyebrow>Pricing</Eyebrow>
          <Heading>One subscription. The whole record.</Heading>
          <Lede>
            No seat tiers, no trade limits, no feature held back for a plan you
            don't have yet.
          </Lede>
        </Reveal>

        <fieldset className="mt-12 flex justify-center">
          <legend className="sr-only">Billing period</legend>
          <div className="inline-flex rounded-lg border border-white/[0.08] bg-white/[0.02] p-1">
            {PLANS.map((option) => (
              <label
                key={option.id}
                className={cn(
                  "cursor-pointer rounded-md px-4 py-1.5 text-sm capitalize transition-colors duration-150",
                  "has-focus-visible:ring-2 has-focus-visible:ring-white/50",
                  cadence === option.id
                    ? "bg-zinc-50 font-medium text-[#0A0A0B]"
                    : "text-zinc-400 hover:text-zinc-100",
                )}
              >
                <input
                  type="radio"
                  name="billing-period"
                  value={option.id}
                  checked={cadence === option.id}
                  onChange={() => setCadence(option.id)}
                  className="sr-only"
                />
                {option.id}
                {option.id === "annual" ? (
                  <span
                    className={cn(
                      "ml-2 rounded px-1.5 py-0.5 font-mono text-[10px] tracking-tight",
                      cadence === "annual"
                        ? "bg-[#0A0A0B]/10 text-[#0A0A0B]"
                        : "bg-profit/15 text-profit",
                    )}
                  >
                    −25%
                  </span>
                ) : null}
              </label>
            ))}
          </div>
        </fieldset>

        <Reveal className="mx-auto mt-10 max-w-md overflow-hidden rounded-2xl border border-white/[0.08] bg-white/[0.015]">
          <div className="border-b border-white/[0.06] px-8 py-8 text-center">
            <p className="flex items-baseline justify-center gap-1.5">
              <span className="font-mono text-5xl font-medium text-zinc-50 tabular-nums">
                <Pending>{plan.price}</Pending>
              </span>
              <span className="text-sm text-zinc-500">{plan.cadence}</span>
            </p>
            <p className="mt-3 text-xs text-zinc-500">{plan.note}</p>
            <SignUpButton>
              <Button
                size="lg"
                onClick={() =>
                  capture(EVENTS.ctaClicked, {
                    location: "pricing",
                    label: plan.cta,
                  })
                }
                className="mt-6 h-11 w-full bg-zinc-50 text-[15px] font-medium text-[#0A0A0B] transition-transform duration-150 hover:bg-zinc-200 active:scale-[0.97]"
              >
                {plan.cta}
              </Button>
            </SignUpButton>
          </div>

          <ul className="space-y-3 px-8 py-7">
            {PLAN_INCLUDES.map((item) => (
              <li key={item} className="flex items-start gap-3">
                <HugeiconsIcon
                  icon={Tick02Icon}
                  strokeWidth={2}
                  className="mt-0.5 size-4 shrink-0 text-zinc-400"
                />
                <span className="text-sm leading-relaxed text-zinc-300">
                  {item}
                </span>
              </li>
            ))}
          </ul>
        </Reveal>
      </Section>
    </div>
  );
}
