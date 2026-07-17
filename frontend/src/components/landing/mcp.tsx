"use client";

import { SCREENSHOTS } from "@/components/landing/content";
import { Reveal } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Section,
  Shot,
} from "@/components/landing/primitives";

const PROMPTS = [
  "Which setup has the best expectancy since April?",
  "Show me every trade where I moved my stop.",
  "What did my breakout playbook actually cost me?",
  "Write tomorrow's plan from my last ten losers.",
];

const CONFIG = `{
  "mcpServers": {
    "tradstry": {
      "url": "https://mcp.tradstry.com"
    }
  }
}`;

export function Mcp() {
  return (
    <Section id="mcp" className="relative overflow-hidden">
      <div
        aria-hidden="true"
        className="pointer-events-none absolute right-[-10rem] top-1/2 size-[36rem] -translate-y-1/2 rounded-full bg-white/[0.05] blur-[130px]"
      />

      <div className="relative grid items-start gap-12 md:grid-cols-2">
        <Reveal>
          <Eyebrow>Model Context Protocol</Eyebrow>
          <Heading>Stop pasting trades into a chat box.</Heading>
          <Lede>
            Tradstry ships an MCP server. Point Claude at it once and your
            journal, playbooks, principles and analytics become things it can
            read and write — on the subscription you already have, with the
            model you already trust.
          </Lede>

          <pre className="mt-8 overflow-x-auto rounded-xl border border-white/[0.08] bg-[#131316] p-4 font-mono text-xs leading-relaxed text-zinc-400">
            <code>{CONFIG}</code>
          </pre>

          <ul className="mt-6 space-y-2.5">
            {PROMPTS.map((prompt) => (
              <li
                key={prompt}
                className="flex items-start gap-2.5 text-sm text-zinc-400"
              >
                <span className="mt-2 size-1 shrink-0 rounded-full bg-zinc-500" />
                <span className="italic">“{prompt}”</span>
              </li>
            ))}
          </ul>
        </Reveal>

        <Reveal className="md:mt-14">
          <Shot shot={SCREENSHOTS.mcp} />
        </Reveal>
      </div>
    </Section>
  );
}
