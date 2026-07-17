import Link from "next/link";
import type * as React from "react";
import { Tape } from "@/components/landing/tape";
import { TradstryMark } from "@/components/logo";

const LEDGER = [
  { setup: "Breakout · continuation", trades: "34 followed", pl: "+$4,180" },
  { setup: "Pullback · trend", trades: "21 followed", pl: "+$1,905" },
  { setup: "Breakout · rules broken", trades: "7 broken", pl: "−$1,240" },
];

export function AuthShell({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: React.ReactNode;
}) {
  return (
    <div
      data-shell="marketing"
      className="dark relative min-h-svh overflow-hidden bg-[#0A0A0B] antialiased"
    >
      <Tape lanes={12} />
      <div
        aria-hidden="true"
        className="pointer-events-none absolute left-1/2 top-[-14rem] size-[44rem] -translate-x-1/2 rounded-full bg-white/[0.04] blur-[140px]"
      />

      <div className="relative mx-auto grid min-h-svh max-w-5xl items-center gap-16 px-6 py-12 lg:grid-cols-[1fr_25rem]">
        <section className="hidden lg:block">
          <Link
            href="/"
            aria-label="Tradstry home"
            className="inline-flex size-10 items-center justify-center rounded-xl bg-zinc-50 text-[#0A0A0B] outline-none transition-transform duration-150 hover:scale-105 focus-visible:ring-2 focus-visible:ring-white/50 active:scale-95"
          >
            <TradstryMark className="size-[22px]" />
          </Link>

          <h1 className="mt-9 text-balance text-4xl font-semibold leading-[1.1] tracking-[-0.03em] text-zinc-50">
            {title}
            <br />
            <span className="text-zinc-500">{subtitle}</span>
          </h1>

          <dl className="mt-12 divide-y divide-white/[0.06] border-y border-white/[0.06]">
            {LEDGER.map((row) => (
              <div
                key={row.setup}
                className="flex items-center justify-between gap-6 py-3.5"
              >
                <dt className="min-w-0">
                  <span className="block truncate text-sm text-zinc-300">
                    {row.setup}
                  </span>
                  <span className="mt-0.5 block text-xs text-zinc-600">
                    {row.trades}
                  </span>
                </dt>
                <dd
                  className={`font-mono text-sm tabular-nums ${
                    row.pl.startsWith("+") ? "text-profit" : "text-loss"
                  }`}
                >
                  {row.pl}
                </dd>
              </div>
            ))}
          </dl>

          <p className="mt-5 text-xs text-zinc-600">
            The same setup, followed and broken. That difference is the product.
          </p>
        </section>

        <section className="mx-auto w-full max-w-sm">
          <Link
            href="/"
            aria-label="Tradstry home"
            className="mb-8 inline-flex size-10 items-center justify-center rounded-xl bg-zinc-50 text-[#0A0A0B] outline-none focus-visible:ring-2 focus-visible:ring-white/50 lg:hidden"
          >
            <TradstryMark className="size-[22px]" />
          </Link>

          <h1 className="mb-7 text-balance text-2xl font-semibold leading-[1.15] tracking-[-0.02em] text-zinc-50 lg:hidden">
            {title} <span className="text-zinc-500">{subtitle}</span>
          </h1>

          {children}

          <p className="mt-8 text-center text-xs leading-relaxed text-zinc-600">
            By continuing you agree to our{" "}
            <Link
              href="/terms"
              className="text-zinc-400 underline underline-offset-4 hover:text-zinc-200"
            >
              Terms
            </Link>{" "}
            and{" "}
            <Link
              href="/privacy"
              className="text-zinc-400 underline underline-offset-4 hover:text-zinc-200"
            >
              Privacy Policy
            </Link>
            .
          </p>
        </section>
      </div>
    </div>
  );
}
