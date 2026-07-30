"use client";

import { PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { motion } from "motion/react";
import { FAQS } from "@/components/landing/content";
import { Reveal, RevealGroup, rise } from "@/components/landing/motion";
import { Eyebrow, Heading, Section } from "@/components/landing/primitives";

export function Faq() {
  return (
    <Section id="faq">
      <div className="grid gap-12 md:grid-cols-[1fr_1.4fr]">
        <Reveal>
          <Eyebrow>FAQ</Eyebrow>
          <Heading className="md:text-4xl">
            Asked honestly. Answered the same way.
          </Heading>
        </Reveal>

        <RevealGroup className="divide-y divide-white/[0.06] border-y border-white/[0.06]">
          {FAQS.map((item) => (
            <motion.details key={item.q} variants={rise} className="group">
              <summary className="flex cursor-pointer list-none items-center justify-between gap-6 py-5 text-[15px] font-medium text-zinc-200 outline-none transition-colors hover:text-zinc-50 focus-visible:text-zinc-50">
                {item.q}
                <HugeiconsIcon
                  icon={PlusSignIcon}
                  strokeWidth={2}
                  className="size-4 shrink-0 text-zinc-500 transition-transform duration-300 ease-out group-open:rotate-45"
                />
              </summary>
              {/* 0fr → 1fr animates a native <details> to auto height; max-height never does this cleanly. */}
              <div className="grid grid-rows-[0fr] transition-[grid-template-rows] duration-300 ease-out group-open:grid-rows-[1fr]">
                <div className="overflow-hidden">
                  <p className="pb-6 pr-10 text-sm leading-relaxed text-zinc-400">
                    {item.a}
                  </p>
                </div>
              </div>
            </motion.details>
          ))}
        </RevealGroup>
      </div>
    </Section>
  );
}
