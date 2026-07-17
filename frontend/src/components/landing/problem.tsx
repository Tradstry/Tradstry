"use client";

import { Cancel01Icon, Tick02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { motion, type Variants } from "motion/react";
import { EASE_OUT } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Section,
} from "@/components/landing/primitives";

/** The rhetoric: both columns land dim, then the answer lights up under your eye, row by row. */
const RESOLVE: Variants = {
  dim: { backgroundColor: "rgba(255,255,255,0)", opacity: 0.45 },
  lit: (index: number) => ({
    backgroundColor: "rgba(255,255,255,0.02)",
    opacity: 1,
    transition: { duration: 0.5, ease: EASE_OUT, delay: 0.25 + index * 0.14 },
  }),
};

const ROWS: Array<{ without: string; with: string }> = [
  {
    without: "Entries typed in by hand, days after the fact",
    with: "Every fill synced from your broker, matched into round trips",
  },
  {
    without: "A rule you keep breaking and can't prove you broke",
    with: "Principles tagged per trade, with the dollar cost of each violation",
  },
  {
    without: "A P&L number that says what, never why",
    with: "Expectancy, SQN, drawdown and R-distribution, sliced by setup",
  },
  {
    without: "Screenshots buried in a Notes app",
    with: "A notebook that syncs offline and links back to the trade",
  },
  {
    without: "Asking an AI about trades you have to paste in first",
    with: "Claude reading your journal directly, over MCP",
  },
];

export function Problem() {
  return (
    <Section>
      <div className="max-w-2xl">
        <Eyebrow>The gap</Eyebrow>
        <Heading>You already have the data. You just can't read it.</Heading>
        <Lede>
          The broker keeps your fills. Your memory keeps the story. Nothing
          keeps both — so the same mistake stays invisible for another quarter.
        </Lede>
      </div>

      <motion.div
        initial="dim"
        whileInView="lit"
        viewport={{ once: true, amount: 0.3 }}
        className="mt-14 overflow-hidden rounded-xl border border-white/[0.08]"
      >
        <div className="grid grid-cols-1 divide-y divide-white/[0.06] md:grid-cols-2 md:divide-x md:divide-y-0">
          <div className="bg-white/[0.01] px-6 py-5">
            <p className="text-xs font-medium uppercase tracking-[0.14em] text-zinc-500">
              A spreadsheet and a good memory
            </p>
          </div>
          <div className="bg-white/[0.04] px-6 py-5">
            <p className="text-xs font-medium uppercase tracking-[0.14em] text-zinc-50">
              Tradstry
            </p>
          </div>
        </div>

        {ROWS.map((row, index) => (
          <div
            key={row.with}
            className="grid grid-cols-1 divide-y divide-white/[0.06] border-t border-white/[0.06] md:grid-cols-2 md:divide-x md:divide-y-0"
          >
            <div className="flex items-start gap-3 px-6 py-5">
              <HugeiconsIcon
                icon={Cancel01Icon}
                strokeWidth={2}
                className="mt-0.5 size-4 shrink-0 text-zinc-600"
              />
              <p className="text-sm leading-relaxed text-zinc-500">
                {row.without}
              </p>
            </div>
            <motion.div
              variants={RESOLVE}
              custom={index}
              className="flex items-start gap-3 px-6 py-5"
            >
              <HugeiconsIcon
                icon={Tick02Icon}
                strokeWidth={2}
                className="mt-0.5 size-4 shrink-0 text-zinc-50"
              />
              <p className="text-sm leading-relaxed text-zinc-200">
                {row.with}
              </p>
            </motion.div>
          </div>
        ))}
      </motion.div>
    </Section>
  );
}
