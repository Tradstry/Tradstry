"use client";

import { motion } from "motion/react";
import {
  EXAMPLE,
  METRICS,
  PLACEHOLDER,
  TESTIMONIAL_PROMPTS,
  TESTIMONIALS,
} from "@/components/landing/content";
import { Reveal, RevealGroup, rise } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Pending,
  Section,
} from "@/components/landing/primitives";
import { cn } from "@tradstry/app-ui/lib/utils";

export function Proof() {
  return (
    <Section>
      <Reveal className="max-w-2xl">
        <Eyebrow>Shipped, not promised</Eyebrow>
        <Heading>Everything on this page already runs.</Heading>
        <Lede>
          No waitlist, no roadmap items dressed up as features. Every number
          below is a fact about the product you would be paying for.
        </Lede>
      </Reveal>

      <RevealGroup
        as="dl"
        className="mt-14 grid grid-cols-2 gap-px overflow-hidden rounded-xl border border-white/[0.08] bg-white/[0.06] md:grid-cols-4"
      >
        {METRICS.map((metric) => (
          <motion.div
            key={metric.label}
            variants={rise}
            className="flex flex-col bg-[#0A0A0B] px-6 py-8"
          >
            <dt className="text-xs uppercase tracking-[0.14em] text-zinc-500">
              {metric.label}
            </dt>
            <dd className="mt-3 font-mono text-3xl text-zinc-50 tabular-nums">
              <Pending>{metric.value}</Pending>
            </dd>
            <p className="mt-3 text-xs leading-relaxed text-zinc-600">
              {metric.note}
            </p>
          </motion.div>
        ))}
      </RevealGroup>

      <Reveal className="mt-6 overflow-hidden rounded-xl border border-white/[0.08] bg-white/[0.015] p-6 md:p-8">
        <div className="max-w-xl">
          <p className="text-balance text-xl font-medium tracking-[-0.01em] text-zinc-50">
            {EXAMPLE.lede}
          </p>
          <p className="mt-3 text-[15px] leading-relaxed text-zinc-400">
            {EXAMPLE.body}
          </p>
        </div>

        <div className="mt-7 grid gap-4 sm:grid-cols-2">
          {EXAMPLE.columns.map((column) => (
            <div
              key={column.title}
              className="rounded-xl border border-white/[0.08] bg-[#131316] p-5"
            >
              <div className="flex items-baseline justify-between gap-3">
                <p className="text-sm font-medium text-zinc-200">
                  {column.title}
                </p>
                <p className="font-mono text-xs text-zinc-600 tabular-nums">
                  {column.count}
                </p>
              </div>
              <dl className="mt-4 grid gap-2.5 border-t border-white/[0.06] pt-4">
                {column.rows.map((row) => (
                  <div
                    key={row.label}
                    className="flex items-center justify-between"
                  >
                    <dt className="text-xs text-zinc-500">{row.label}</dt>
                    <dd
                      className={cn(
                        "font-mono text-sm tabular-nums",
                        column.tone === "profit" ? "text-profit" : "text-loss",
                      )}
                    >
                      {row.value}
                    </dd>
                  </div>
                ))}
              </dl>
            </div>
          ))}
        </div>

        <p className="mt-5 max-w-2xl text-xs leading-relaxed text-zinc-600">
          {EXAMPLE.footnote}
        </p>
      </Reveal>

      <RevealGroup as="ul" className="mt-6 grid gap-6 md:grid-cols-3">
        {TESTIMONIALS.map((item, index) => (
          <motion.li
            key={TESTIMONIAL_PROMPTS[index]}
            variants={rise}
            className="flex flex-col rounded-xl border border-white/[0.08] bg-white/[0.015] p-6"
          >
            <blockquote className="flex-1 text-[15px] leading-relaxed text-zinc-300">
              <Pending>{item.quote}</Pending>
              {item.quote === PLACEHOLDER ? (
                <p className="mt-3 text-xs leading-relaxed text-zinc-600 italic">
                  {TESTIMONIAL_PROMPTS[index]}
                </p>
              ) : null}
            </blockquote>
            <footer className="mt-6 flex items-center gap-2 border-t border-white/[0.06] pt-4">
              <span className="text-sm font-medium text-zinc-200">
                <Pending>{item.name}</Pending>
              </span>
              <span className="text-xs text-zinc-500">
                <Pending>{item.role}</Pending>
              </span>
            </footer>
          </motion.li>
        ))}
      </RevealGroup>
    </Section>
  );
}
