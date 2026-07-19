"use client";

import { PlusSignIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { motion } from "motion/react";
import { Reveal, RevealGroup, rise } from "@/components/landing/motion";
import { Eyebrow, Heading, Section } from "@/components/landing/primitives";

const FAQS = [
  {
    q: "Which brokers can I connect?",
    a: "Anything SnapTrade supports — 35+ institutions including Schwab, Fidelity, Interactive Brokers, Robinhood, E*TRADE, Webull, Coinbase and eToro. Tradstry pulls your executions and matches them into round trips. You can also add trades by hand if your broker isn't covered.",
  },
  {
    q: "What is MCP, in one sentence?",
    a: "It's the protocol Claude uses to talk to outside tools. Point Claude at Tradstry's MCP server and it can read your trades, analytics, playbooks and notebook — and write to them — without you pasting anything.",
  },
  {
    q: "Does the AI cost extra?",
    a: "Over MCP, no — you use your own Claude subscription and your own model, so you're not paying us a margin on tokens. The AI built into the app (chat, summarise, rewrite) runs on ours, so it's metered: 25 actions a month on Free, 300 on Pro, 1,500 on Pro Plus. Autocomplete is free and unmetered.",
  },
  {
    q: "Does it work offline?",
    a: "The desktop app does. It keeps a local database, lets you journal on a plane, and merges cleanly when you reconnect — no last-write-wins data loss.",
  },
  {
    q: "Who owns my data?",
    a: "You do. Export it whenever you like, and deleting your account erases it. We don't sell it, and we don't train on it.",
  },
  {
    q: "Can I cancel?",
    a: "Any time, from the account dialog. You keep access until the end of the period you paid for.",
  },
];

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
