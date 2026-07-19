"use client";

import { SignUpButton } from "@clerk/nextjs";
import { Tick02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  PLAN_INCLUDES,
  PLAN_LIMITS,
  PLANS,
  type Plan,
} from "@/components/landing/content";
import { Reveal, RevealGroup } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Pending,
  Section,
} from "@/components/landing/primitives";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";

function PlanCard({ plan }: { plan: Plan }) {
  return (
    <div
      className={cn(
        "flex flex-col overflow-hidden rounded-2xl border",
        plan.featured
          ? "border-white/20 bg-white/[0.04]"
          : "border-white/[0.08] bg-white/[0.015]",
      )}
    >
      <div className="px-6 py-7">
        <div className="flex items-center gap-2">
          <p className="text-sm font-medium text-zinc-200">{plan.name}</p>
          {plan.featured ? (
            <span className="rounded bg-zinc-50/90 px-1.5 py-0.5 font-mono text-[10px] tracking-tight text-[#0A0A0B]">
              Most picked
            </span>
          ) : null}
        </div>

        <p className="mt-4 flex items-baseline gap-1.5">
          <span className="font-mono text-4xl font-medium text-zinc-50 tabular-nums">
            <Pending>{plan.price}</Pending>
          </span>
          {plan.cadence ? (
            <span className="text-sm text-zinc-500">{plan.cadence}</span>
          ) : null}
        </p>
        <p className="mt-3 text-xs text-zinc-500">{plan.note}</p>

        <SignUpButton>
          <Button
            size="lg"
            className={cn(
              "mt-6 h-11 w-full text-[15px] font-medium transition-transform duration-150 active:scale-[0.97]",
              plan.featured
                ? "bg-zinc-50 text-[#0A0A0B] hover:bg-zinc-200"
                : "border border-white/15 bg-transparent text-zinc-100 hover:bg-white/[0.06]",
            )}
          >
            {plan.cta}
          </Button>
        </SignUpButton>
      </div>
    </div>
  );
}

export function Pricing() {
  return (
    <Section id="pricing">
      <Reveal className="max-w-2xl">
        <Eyebrow>Pricing</Eyebrow>
        <Heading>Every plan. Every feature.</Heading>
        <Lede>
          Nothing is held back behind a tier — the analytics, the playbooks, the
          brokerage sync, the desktop app and the MCP server are in all three.
          Plans differ only in how much you use.
        </Lede>
      </Reveal>

      <RevealGroup className="mt-12 grid gap-4 md:grid-cols-3">
        {PLANS.map((plan) => (
          <PlanCard key={plan.id} plan={plan} />
        ))}
      </RevealGroup>

      <Reveal className="mt-4 overflow-hidden rounded-2xl border border-white/[0.08] bg-white/[0.015]">
        <div className="overflow-x-auto">
          <table className="w-full min-w-[34rem] text-sm">
            <caption className="sr-only">Monthly limits by plan</caption>
            <thead>
              <tr className="border-b border-white/[0.06]">
                <th
                  scope="col"
                  className="px-6 py-4 text-left text-xs font-medium uppercase tracking-[0.14em] text-zinc-500"
                >
                  Limits
                </th>
                {PLANS.map((plan) => (
                  <th
                    key={plan.id}
                    scope="col"
                    className={cn(
                      "px-6 py-4 text-right text-xs font-medium",
                      plan.featured ? "text-zinc-200" : "text-zinc-500",
                    )}
                  >
                    {plan.name}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {PLAN_LIMITS.map((limit) => (
                <tr
                  key={limit.label}
                  className="border-b border-white/[0.04] last:border-0"
                >
                  <th
                    scope="row"
                    className="px-6 py-4 text-left font-normal text-zinc-400"
                  >
                    {limit.label}
                  </th>
                  <td className="px-6 py-4 text-right font-mono text-zinc-300 tabular-nums">
                    {limit.free}
                  </td>
                  <td className="px-6 py-4 text-right font-mono text-zinc-50 tabular-nums">
                    {limit.pro}
                  </td>
                  <td className="px-6 py-4 text-right font-mono text-zinc-300 tabular-nums">
                    {limit.proPlus}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>

        <ul className="grid gap-3 border-t border-white/[0.06] px-6 py-7 sm:grid-cols-2">
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
  );
}
