"use client";

import { animate, motion, useInView, useReducedMotion } from "motion/react";
import * as React from "react";
import { EASE_OUT, Reveal } from "@/components/landing/motion";
import {
  Eyebrow,
  Heading,
  Lede,
  Section,
} from "@/components/landing/primitives";
import { cn } from "@/lib/utils";

type Leak = { annual: number; recovered: number };

const LeakContext = React.createContext<Leak>({ annual: 0, recovered: 0 });

export const useLeak = () => React.useContext(LeakContext);

const DEFAULTS = { trades: 40, breakRate: 20, avgLoss: 250 };

const money = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});

/** Their numbers, multiplied out. We assert nothing — the inputs are the user's own confession. */
function annualLeak(trades: number, breakRate: number, avgLoss: number) {
  return trades * 12 * (breakRate / 100) * avgLoss;
}

export function LeakProvider({ children }: { children: React.ReactNode }) {
  const [trades, setTrades] = React.useState(DEFAULTS.trades);
  const [breakRate, setBreakRate] = React.useState(DEFAULTS.breakRate);
  const [avgLoss, setAvgLoss] = React.useState(DEFAULTS.avgLoss);

  const annual = annualLeak(trades, breakRate, avgLoss);
  const value = React.useMemo(
    () => ({ annual, recovered: annual / 2 }),
    [annual],
  );

  return (
    <LeakContext.Provider value={value}>
      <LeakInputs.Provider
        value={{
          trades,
          setTrades,
          breakRate,
          setBreakRate,
          avgLoss,
          setAvgLoss,
        }}
      >
        {children}
      </LeakInputs.Provider>
    </LeakContext.Provider>
  );
}

const LeakInputs = React.createContext<{
  trades: number;
  setTrades: (v: number) => void;
  breakRate: number;
  setBreakRate: (v: number) => void;
  avgLoss: number;
  setAvgLoss: (v: number) => void;
}>({
  trades: DEFAULTS.trades,
  setTrades: () => {},
  breakRate: DEFAULTS.breakRate,
  setBreakRate: () => {},
  avgLoss: DEFAULTS.avgLoss,
  setAvgLoss: () => {},
});

function Dial({
  label,
  hint,
  value,
  display,
  min,
  max,
  step,
  onChange,
}: {
  label: string;
  hint: string;
  value: number;
  display: string;
  min: number;
  max: number;
  step: number;
  onChange: (v: number) => void;
}) {
  const id = React.useId();
  return (
    <div className="grid gap-2">
      <div className="flex items-baseline justify-between gap-4">
        <label htmlFor={id} className="text-sm text-zinc-300">
          {label}
        </label>
        <span className="font-mono text-sm text-zinc-50 tabular-nums">
          {display}
        </span>
      </div>
      <input
        id={id}
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        className={cn(
          "h-1 w-full cursor-pointer appearance-none rounded-full bg-white/10 outline-none",
          "focus-visible:ring-2 focus-visible:ring-white/50 focus-visible:ring-offset-2 focus-visible:ring-offset-[#0A0A0B]",
          "[&::-webkit-slider-thumb]:size-4 [&::-webkit-slider-thumb]:appearance-none [&::-webkit-slider-thumb]:rounded-full [&::-webkit-slider-thumb]:bg-zinc-50 [&::-webkit-slider-thumb]:transition-transform [&::-webkit-slider-thumb]:duration-150 [&::-webkit-slider-thumb]:hover:scale-110",
          "[&::-moz-range-thumb]:size-4 [&::-moz-range-thumb]:appearance-none [&::-moz-range-thumb]:rounded-full [&::-moz-range-thumb]:border-0 [&::-moz-range-thumb]:bg-zinc-50",
        )}
      />
      <p className="text-xs text-zinc-600">{hint}</p>
    </div>
  );
}

function Figure({ value, className }: { value: number; className?: string }) {
  const ref = React.useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.6 });
  const reduced = useReducedMotion();
  const rolled = React.useRef(false);

  React.useEffect(() => {
    const node = ref.current;
    if (!node) return;

    // Roll up once, on arrival. After that the dials drive it, so it must track instantly.
    if (!inView || rolled.current || reduced) {
      node.textContent = money.format(value);
      if (inView) rolled.current = true;
      return;
    }
    rolled.current = true;
    const controls = animate(0, value, {
      duration: 1.1,
      ease: EASE_OUT,
      onUpdate: (v) => {
        node.textContent = money.format(v);
      },
    });
    return () => controls.stop();
  }, [inView, value, reduced]);

  return (
    <span ref={ref} className={className}>
      {money.format(0)}
    </span>
  );
}

export function LeakSection() {
  const { trades, setTrades, breakRate, setBreakRate, avgLoss, setAvgLoss } =
    React.useContext(LeakInputs);
  const { annual, recovered } = useLeak();

  const breaking = Math.round(trades * (breakRate / 100));

  return (
    <Section id="leak">
      <Reveal className="max-w-2xl">
        <Eyebrow>The leak</Eyebrow>
        <Heading>
          Your worst trades weren't losses. They were rule-breaks.
        </Heading>
        <Lede>
          Losing trades are the cost of doing business. The trades you took
          against your own rules are something else — and they have a price you
          have never once been shown. Put your numbers in.
        </Lede>
      </Reveal>

      <div className="mt-14 grid items-stretch gap-6 md:grid-cols-[1fr_1fr]">
        <Reveal className="rounded-xl border border-white/[0.08] bg-white/[0.015] p-6 md:p-7">
          <div className="grid gap-7">
            <Dial
              label="Trades per month"
              hint="Round trips, not fills."
              value={trades}
              display={String(trades)}
              min={5}
              max={200}
              step={5}
              onChange={setTrades}
            />
            <Dial
              label="Share that break your own rules"
              hint={`About ${breaking} trade${breaking === 1 ? "" : "s"} a month. Be honest — nobody's watching.`}
              value={breakRate}
              display={`${breakRate}%`}
              min={5}
              max={50}
              step={1}
              onChange={setBreakRate}
            />
            <Dial
              label="Average loss on those"
              hint="What a bad one usually costs you."
              value={avgLoss}
              display={money.format(avgLoss)}
              min={50}
              max={5000}
              step={50}
              onChange={setAvgLoss}
            />
          </div>
        </Reveal>

        <Reveal className="flex flex-col justify-center rounded-xl border border-white/[0.08] bg-[#131316] p-6 md:p-7">
          <p className="text-xs uppercase tracking-[0.14em] text-zinc-500">
            Breaking your rules costs you
          </p>
          <Figure
            value={annual}
            className="mt-3 font-mono text-4xl text-loss tabular-nums md:text-5xl"
          />
          <p className="mt-1.5 text-sm text-zinc-500">every year</p>

          <div className="mt-7 border-t border-white/[0.06] pt-6">
            <p className="text-xs uppercase tracking-[0.14em] text-zinc-500">
              Cut half of them and you keep
            </p>
            <Figure
              value={recovered}
              className="mt-3 font-mono text-2xl text-profit tabular-nums"
            />
            <p className="mt-3 text-xs leading-relaxed text-zinc-600">
              This is arithmetic on the numbers you just entered, not a
              projection. Tradstry does not predict returns — it prices what
              already happened, so you can see the trades you keep taking
              anyway.
            </p>
          </div>
        </Reveal>
      </div>
    </Section>
  );
}

/** The playbook card links here rather than duplicating the dials. */
export function LeakLink() {
  return (
    <motion.a
      href="#leak"
      whileHover={{ x: 2 }}
      transition={{ duration: 0.15, ease: EASE_OUT }}
      className="mt-4 inline-flex items-center gap-1.5 text-xs text-zinc-500 transition-colors hover:text-zinc-200"
    >
      Price your own leak
      <span aria-hidden="true">→</span>
    </motion.a>
  );
}
