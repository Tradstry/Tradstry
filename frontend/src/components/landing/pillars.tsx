"use client";

import {
  AnalyticsUpIcon,
  BookOpen01Icon,
  File01Icon,
  Notebook01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import type * as React from "react";
import { EquityCard, PlaybookCard } from "@/components/landing/cards";
import { SCREENSHOTS } from "@/components/landing/content";
import { Reveal } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Section,
  Shot,
} from "@/components/landing/primitives";

const PILLARS = [
  {
    icon: File01Icon,
    kicker: "Journal",
    title: "Every fill, already there.",
    body: "Connect a brokerage and Tradstry pulls your executions, matches them into round trips, and works out P&L, R-multiple and holding time before you open the app. Tag the setup, write the note, attach the chart.",
    visual: <Shot shot={SCREENSHOTS.journal} />,
  },
  {
    icon: AnalyticsUpIcon,
    kicker: "Analytics",
    title: "The number, and the reason.",
    body: "Expectancy in dollars and in R. SQN, recovery factor, max drawdown against real account equity. Broken out by symbol, session, day of week and playbook — so an edge stops being a feeling.",
    visual: <EquityCard />,
  },
  {
    icon: BookOpen01Icon,
    kicker: "Playbook",
    title: "The rules you wrote, enforced.",
    body: "Write a setup as a numbered playbook. Attach the principles you refuse to break. Every trade gets checked against them, and the ones you broke get a dollar figure next to them.",
    visual: <PlaybookCard />,
  },
  {
    icon: Notebook01Icon,
    kicker: "Notebook",
    title: "Thinking, not filing.",
    body: "A real editor — images, code, slash commands, and an autocomplete that has read your journal and finishes the sentence you were already writing. Syncs across web and desktop, keeps working on a plane, and links a note to the trade it explains so the two stay together.",
    visual: <Shot shot={SCREENSHOTS.notebook} />,
  },
] satisfies Array<{
  icon: typeof File01Icon;
  kicker: string;
  title: string;
  body: string;
  visual: React.ReactNode;
}>;

export function Pillars() {
  return (
    <Section id="product">
      <Reveal className="max-w-2xl">
        <Eyebrow>The product</Eyebrow>
        <Heading>Four pieces. One record.</Heading>
        <Lede>
          Not four apps you glue together at the end of the week. One record
          that every part reads from and writes to.
        </Lede>
      </Reveal>

      <div className="mt-14 space-y-6">
        {PILLARS.map((pillar, index) => (
          <Reveal
            key={pillar.kicker}
            as="article"
            className="grid items-center gap-8 overflow-hidden rounded-xl border border-white/[0.08] bg-white/[0.015] p-6 md:grid-cols-2 md:p-8"
          >
            <div className={index % 2 === 1 ? "md:order-2" : undefined}>
              <div className="flex items-center gap-2.5">
                <span className="flex size-8 items-center justify-center rounded-lg border border-white/[0.08] bg-white/[0.06] text-zinc-300">
                  <HugeiconsIcon
                    icon={pillar.icon}
                    strokeWidth={2}
                    className="size-4"
                  />
                </span>
                <span className="text-[11px] font-medium uppercase tracking-[0.16em] text-zinc-500">
                  {pillar.kicker}
                </span>
              </div>
              <h3 className="mt-5 text-balance text-2xl font-semibold tracking-[-0.02em] text-zinc-50">
                {pillar.title}
              </h3>
              <p className="mt-3 text-[15px] leading-relaxed text-zinc-400">
                {pillar.body}
              </p>
            </div>

            <div className={index % 2 === 1 ? "md:order-1" : undefined}>
              {pillar.visual}
            </div>
          </Reveal>
        ))}
      </div>
    </Section>
  );
}
