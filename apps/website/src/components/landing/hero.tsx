"use client";

import { SignUpButton } from "@clerk/nextjs";
import { motion } from "motion/react";
import { SCREENSHOTS } from "@/components/landing/content";
import { EASE_OUT, PrintedLine } from "@/components/landing/motion";
import { Shot } from "@/components/landing/primitives";
import { Tape } from "@/components/landing/tape";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { capture, EVENTS } from "@/lib/analytics/events";

export function Hero() {
  return (
    <section id="top" className="relative overflow-hidden pt-20 pb-24 md:pt-28">
      <Tape className="h-[46rem]" lanes={11} />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute left-1/2 top-[-12rem] size-[42rem] -translate-x-1/2 rounded-full bg-white/[0.045] blur-[140px]"
      />

      <div className="relative mx-auto max-w-6xl px-6">
        <div className="mx-auto max-w-3xl text-center">
          <motion.a
            href="#mcp"
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.4, ease: EASE_OUT }}
            className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-white/[0.03] py-1 pl-1 pr-3 text-xs text-zinc-400 transition-colors hover:border-white/20 hover:text-zinc-200"
          >
            <span className="rounded-full bg-zinc-50 px-2 py-0.5 font-medium text-[#0A0A0B]">
              New
            </span>
            Query your journal from Claude over MCP
          </motion.a>

          <h1 className="mt-7 text-balance text-[2.5rem] font-semibold leading-[1.05] tracking-[-0.03em] text-zinc-50 md:text-6xl">
            <PrintedLine delay={0.12}>Your trades are a record.</PrintedLine>
            <PrintedLine delay={0.21} className="text-zinc-500">
              Most traders never read it.
            </PrintedLine>
          </h1>

          <motion.p
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, ease: EASE_OUT, delay: 0.42 }}
            className="mx-auto mt-6 max-w-xl text-pretty text-[17px] leading-relaxed text-zinc-400"
          >
            Tradstry syncs every fill from your broker, holds you to the rules
            you wrote, and reconstructs what your account actually did — then
            hands the whole thing to the AI you already pay for.
          </motion.p>

          <motion.div
            initial={{ opacity: 0, y: 8 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.5, ease: EASE_OUT, delay: 0.54 }}
            className="mt-9 flex flex-col items-center justify-center gap-3 sm:flex-row"
          >
            <SignUpButton>
              <Button
                size="lg"
                onClick={() =>
                  capture(EVENTS.ctaClicked, {
                    location: "hero",
                    label: "Start journalling",
                  })
                }
                className="h-11 w-full bg-zinc-50 px-7 text-[15px] text-[#0A0A0B] transition-transform duration-150 hover:bg-zinc-200 active:scale-[0.97] sm:w-auto"
              >
                Start journalling
              </Button>
            </SignUpButton>
            <Button
              asChild
              size="lg"
              variant="outline"
              className="h-11 w-full border-white/10 bg-transparent px-7 text-[15px] text-zinc-300 transition-transform duration-150 hover:bg-white/5 hover:text-zinc-50 active:scale-[0.97] sm:w-auto"
            >
              <a href="#product">See what's inside</a>
            </Button>
          </motion.div>
        </div>

        <motion.div
          initial={{ opacity: 0, y: 24 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.8, ease: EASE_OUT, delay: 0.66 }}
          className="relative mt-16 md:mt-20"
        >
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-x-8 -top-6 h-24 rounded-full bg-white/[0.06] blur-3xl"
          />
          <Shot shot={SCREENSHOTS.workspace} className="relative" />
        </motion.div>
      </div>
    </section>
  );
}
