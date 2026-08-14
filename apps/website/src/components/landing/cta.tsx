"use client";

import { SignUpButton } from "@clerk/nextjs";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { motion } from "motion/react";
import { EASE_OUT } from "@/components/landing/motion";
import { Tape } from "@/components/landing/tape";
import { capture, EVENTS } from "@/lib/analytics/events";

const VIEWPORT = { once: true, amount: 0.6 } as const;

/** The whole product in 1.2s: the ledger exists, then the trade cuts across it. Used once. */
function DrawnMark() {
  return (
    <motion.svg
      viewBox="0 0 32 32"
      fill="none"
      aria-hidden="true"
      className="size-6"
      initial="hidden"
      whileInView="shown"
      viewport={VIEWPORT}
    >
      {["M7 12h18", "M7 17h18"].map((d, index) => (
        <motion.path
          key={d}
          d={d}
          stroke="currentColor"
          strokeWidth={1.5}
          strokeLinecap="round"
          variants={{
            hidden: { opacity: 0 },
            shown: {
              opacity: 0.35,
              transition: {
                duration: 0.3,
                delay: index * 0.08,
                ease: EASE_OUT,
              },
            },
          }}
        />
      ))}
      <motion.path
        d="M7 23l6-6 4 3 8-11"
        stroke="currentColor"
        strokeWidth={2.6}
        strokeLinecap="round"
        strokeLinejoin="round"
        variants={{
          hidden: { pathLength: 0 },
          shown: {
            pathLength: 1,
            transition: { duration: 0.9, delay: 0.26, ease: EASE_OUT },
          },
        }}
      />
    </motion.svg>
  );
}

export function Cta() {
  return (
    <section className="relative overflow-hidden border-t border-white/[0.06] py-28 md:py-36">
      <Tape lanes={9} />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute left-1/2 top-1/2 size-[34rem] -translate-x-1/2 -translate-y-1/2 rounded-full bg-white/[0.055] blur-[130px]"
      />

      <div className="relative mx-auto max-w-2xl px-6 text-center">
        <motion.span
          initial={{ opacity: 0, scale: 0.92 }}
          whileInView={{ opacity: 1, scale: 1 }}
          viewport={VIEWPORT}
          transition={{ duration: 0.45, ease: EASE_OUT }}
          className="inline-flex size-11 items-center justify-center rounded-xl bg-zinc-50 text-[#0A0A0B]"
        >
          <DrawnMark />
        </motion.span>

        <motion.h2
          initial={{ opacity: 0, y: 10 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={VIEWPORT}
          transition={{ duration: 0.5, delay: 0.5, ease: EASE_OUT }}
          className="mt-8 text-balance text-3xl font-semibold tracking-[-0.02em] text-zinc-50 md:text-[2.75rem] md:leading-[1.1]"
        >
          <span className="block">Plan the risk. Sync the execution.</span>
          <span className="block text-zinc-500">
            Find the deviation. Improve the process.
          </span>
        </motion.h2>

        <motion.p
          initial={{ opacity: 0, y: 10 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={VIEWPORT}
          transition={{ duration: 0.5, delay: 0.62, ease: EASE_OUT }}
          className="mx-auto mt-5 max-w-md text-pretty text-[15px] leading-relaxed text-zinc-400"
        >
          Tradstry connects your plans, broker fills, trading rules, and
          journal—so every trade shows you what to repeat and what to change.
        </motion.p>

        <motion.div
          initial={{ opacity: 0, y: 10 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={VIEWPORT}
          transition={{ duration: 0.5, delay: 0.74, ease: EASE_OUT }}
        >
          <SignUpButton>
            <Button
              size="lg"
              onClick={() =>
                capture(EVENTS.ctaClicked, {
                  location: "footer_cta",
                  label: "Start journalling",
                })
              }
              className="mt-9 h-11 bg-zinc-50 px-8 text-[15px] text-[#0A0A0B] transition-transform duration-150 hover:bg-zinc-200 active:scale-[0.97]"
            >
              Start journalling
            </Button>
          </SignUpButton>
        </motion.div>
      </div>
    </section>
  );
}
