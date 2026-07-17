"use client";

import { animate, motion, useInView, useReducedMotion } from "motion/react";
import * as React from "react";
import { LeakLink } from "@/components/landing/leak";
import { EASE_OUT } from "@/components/landing/motion";

const RULES = [
  "Price above the 20 EMA on the daily",
  "Volume at least 1.5× the 50-day average",
  "Stop below the pivot low, never below 1R",
];

const money = new Intl.NumberFormat("en-US", {
  style: "currency",
  currency: "USD",
  maximumFractionDigits: 0,
});

function Rolling({
  to,
  sign,
  className,
}: {
  to: number;
  sign: "+" | "−";
  className?: string;
}) {
  const ref = React.useRef<HTMLSpanElement>(null);
  const inView = useInView(ref, { once: true, amount: 0.6 });
  const reduced = useReducedMotion();

  React.useEffect(() => {
    const node = ref.current;
    if (!node) return;
    if (!inView) return;
    if (reduced) {
      node.textContent = `${sign}${money.format(to)}`;
      return;
    }
    const controls = animate(0, to, {
      duration: 1.1,
      ease: EASE_OUT,
      onUpdate: (value) => {
        node.textContent = `${sign}${money.format(value)}`;
      },
    });
    return () => controls.stop();
  }, [inView, to, sign, reduced]);

  return (
    <span ref={ref} className={className}>
      {sign}
      {money.format(0)}
    </span>
  );
}

export function PlaybookCard() {
  return (
    <div className="rounded-xl border border-white/10 bg-[#131316] p-5">
      <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-zinc-500">
        Breakout · continuation
      </p>
      <ol className="mt-4 space-y-2.5">
        {RULES.map((rule, index) => (
          <li key={rule} className="flex gap-3 text-sm text-zinc-300">
            <span className="font-mono text-xs text-zinc-500 tabular-nums">
              {String(index + 1).padStart(2, "0")}
            </span>
            {rule}
          </li>
        ))}
      </ol>
      <dl className="mt-5 grid gap-2 border-t border-white/[0.06] pt-4">
        <div className="flex items-center justify-between">
          <dt className="text-xs text-zinc-500">Followed · 34 trades</dt>
          <dd>
            <Rolling
              to={4180}
              sign="+"
              className="font-mono text-sm text-profit tabular-nums"
            />
          </dd>
        </div>
        <div className="flex items-center justify-between">
          <dt className="text-xs text-zinc-500">Broken · 7 trades</dt>
          <dd>
            <Rolling
              to={1240}
              sign="−"
              className="font-mono text-sm text-loss tabular-nums"
            />
          </dd>
        </div>
      </dl>
      <LeakLink />
    </div>
  );
}

const CURVE =
  "M0 88 L40 78 80 84 120 62 160 70 200 46 240 55 280 34 320 40 360 20 400 14";
const AREA = `M0 96 L0 88 40 78 80 84 120 62 160 70 200 46 240 55 280 34 320 40 360 20 400 14 L400 110 L0 110 Z`;

const VIEWPORT = { once: true, amount: 0.5 } as const;

/** Time is the x-axis, so the honest animation is to draw it in time. */
export function EquityCard() {
  return (
    <div className="rounded-xl border border-white/10 bg-[#131316] p-5">
      <div className="flex items-baseline justify-between">
        <p className="text-[11px] font-medium uppercase tracking-[0.16em] text-zinc-500">
          Account equity
        </p>
        <Rolling
          to={38420}
          sign="+"
          className="font-mono text-sm text-profit tabular-nums"
        />
      </div>

      <svg
        viewBox="0 0 400 110"
        preserveAspectRatio="none"
        role="img"
        aria-label="Account equity rising over time"
        className="mt-4 h-28 w-full overflow-visible"
      >
        <title>Account equity rising over time</title>
        <defs>
          <linearGradient id="equity-fill" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0%" stopColor="#fff" stopOpacity="0.1" />
            <stop offset="100%" stopColor="#fff" stopOpacity="0" />
          </linearGradient>
        </defs>
        <motion.path
          d={AREA}
          fill="url(#equity-fill)"
          initial={{ opacity: 0 }}
          whileInView={{ opacity: 1 }}
          viewport={VIEWPORT}
          transition={{ duration: 0.7, delay: 0.7, ease: EASE_OUT }}
        />
        <motion.path
          d={CURVE}
          fill="none"
          stroke="#fafafa"
          strokeWidth={1.5}
          strokeLinejoin="round"
          vectorEffect="non-scaling-stroke"
          initial={{ pathLength: 0 }}
          whileInView={{ pathLength: 1 }}
          viewport={VIEWPORT}
          transition={{ duration: 1.5, ease: EASE_OUT }}
        />
        <motion.circle
          cx={400}
          cy={14}
          r={3}
          fill="#fafafa"
          initial={{ opacity: 0, scale: 0.4 }}
          whileInView={{ opacity: 1, scale: 1 }}
          viewport={VIEWPORT}
          transition={{ duration: 0.38, delay: 1.4, ease: EASE_OUT }}
        />
      </svg>

      <dl className="mt-4 grid grid-cols-3 gap-3 border-t border-white/[0.06] pt-4">
        {[
          { label: "Expectancy", value: "0.42R" },
          { label: "SQN", value: "2.7" },
          { label: "Max DD", value: "−8.2%" },
        ].map((stat) => (
          <div key={stat.label}>
            <dt className="text-[11px] text-zinc-600">{stat.label}</dt>
            <dd className="mt-0.5 font-mono text-sm text-zinc-200 tabular-nums">
              {stat.value}
            </dd>
          </div>
        ))}
      </dl>
    </div>
  );
}
