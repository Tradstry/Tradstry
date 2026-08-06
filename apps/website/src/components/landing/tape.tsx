"use client";

import { useScroll, useVelocity } from "motion/react";
import * as React from "react";
import { cn } from "@/lib/utils";

const INK = "10, 10, 11";
const SYMBOLS = [
  "NVDA",
  "AAPL",
  "TSLA",
  "SPY",
  "AMD",
  "MSFT",
  "META",
  "QQQ",
  "AMZN",
  "COIN",
  "PLTR",
  "AVGO",
];

/** Seeded so the tape reads the same on every load — it's a record, not a slot machine. */
function rng(seed: number) {
  let s = seed >>> 0;
  return () => {
    s = (s * 1664525 + 1013904223) >>> 0;
    return s / 4294967296;
  };
}

type Lane = { text: string; speed: number; offset: number; alpha: number };

function buildLanes(count: number): Lane[] {
  const rand = rng(3);
  return Array.from({ length: count }, (_, i) => {
    const fills = Array.from({ length: 14 }, () => {
      const symbol = SYMBOLS[Math.floor(rand() * SYMBOLS.length)];
      const side = rand() > 0.5 ? "BUY" : "SELL";
      const qty = String(Math.floor(rand() * 900 + 20)).padStart(3, "0");
      const price = (rand() * 400 + 20).toFixed(2);
      return `${symbol}  ${side}  ${qty} @ ${price}`;
    });
    return {
      text: fills.join("      ·      "),
      speed: 36 + rand() * 96,
      offset: rand() * 600,
      alpha: 0.05 + (i % 3) * 0.022,
    };
  });
}

export function Tape({
  className,
  lanes: laneCount = 9,
}: {
  className?: string;
  lanes?: number;
}) {
  const ref = React.useRef<HTMLCanvasElement>(null);
  const { scrollY } = useScroll();
  const scrollVelocity = useVelocity(scrollY);

  React.useEffect(() => {
    const canvas = ref.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const lanes = buildLanes(laneCount);
    const reduced = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;

    let width = 0;
    let height = 0;
    let visible = true;
    let frame = 0;
    let last = performance.now();
    let boost = 0;

    const unsubscribe = scrollVelocity.on("change", (v) => {
      boost = Math.min(boost + Math.abs(v) * 0.06, 900);
    });

    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      const rect = canvas.getBoundingClientRect();
      width = rect.width;
      height = rect.height;
      canvas.width = Math.round(width * dpr);
      canvas.height = Math.round(height * dpr);
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    };

    const draw = (now: number) => {
      const dt = Math.min((now - last) / 1000, 0.05);
      last = now;
      boost *= 0.94;

      ctx.clearRect(0, 0, width, height);
      ctx.font = "12px var(--font-geist-mono), ui-monospace, monospace";
      ctx.textBaseline = "middle";

      const gap = height / (lanes.length + 1);
      for (const [i, lane] of lanes.entries()) {
        if (!reduced) lane.offset += (lane.speed + boost) * dt;
        const run = ctx.measureText(lane.text).width;
        if (run === 0) continue;
        const x = -(lane.offset % run);
        const y = gap * (i + 1);
        ctx.fillStyle = `rgba(255, 255, 255, ${lane.alpha})`;
        ctx.fillText(lane.text, x, y);
        ctx.fillText(lane.text, x + run, y);
      }

      const veil = ctx.createRadialGradient(
        width / 2,
        height / 2,
        0,
        width / 2,
        height / 2,
        Math.max(width, height) * 0.55,
      );
      veil.addColorStop(0, `rgba(${INK}, 0.92)`);
      veil.addColorStop(0.45, `rgba(${INK}, 0.72)`);
      veil.addColorStop(1, `rgba(${INK}, 0)`);
      ctx.fillStyle = veil;
      ctx.fillRect(0, 0, width, height);

      if (!reduced && visible) frame = requestAnimationFrame(draw);
    };

    const start = () => {
      cancelAnimationFrame(frame);
      last = performance.now();
      frame = requestAnimationFrame(draw);
    };

    resize();
    start();

    const observer = new ResizeObserver(() => {
      resize();
      if (reduced || !visible) draw(performance.now());
    });
    observer.observe(canvas);

    // A tape nobody can see should not cost a frame.
    const seen = new IntersectionObserver(([entry]) => {
      visible = entry.isIntersecting;
      if (visible && !reduced) start();
      else cancelAnimationFrame(frame);
    });
    seen.observe(canvas);

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
      seen.disconnect();
      unsubscribe();
    };
  }, [laneCount, scrollVelocity]);

  return (
    <div
      aria-hidden="true"
      className={cn(
        "pointer-events-none absolute inset-0 overflow-hidden",
        className,
      )}
    >
      <canvas ref={ref} className="size-full" />
    </div>
  );
}
