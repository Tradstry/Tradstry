"use client";

import { motion, useReducedMotion, type Variants } from "motion/react";
import type * as React from "react";

export const EASE_OUT = [0.16, 1, 0.3, 1] as const;

/** Every entrance on the page runs on these two variants, so the whole thing shares one rhythm. */
export const stagger: Variants = {
  hidden: {},
  shown: { transition: { staggerChildren: 0.055 } },
};

export const rise: Variants = {
  hidden: { opacity: 0, y: 10 },
  shown: {
    opacity: 1,
    y: 0,
    transition: { duration: 0.42, ease: EASE_OUT },
  },
};

const VIEWPORT = { once: true, amount: 0.35 } as const;

export function Reveal({
  children,
  className,
  as = "div",
}: {
  children: React.ReactNode;
  className?: string;
  as?: "div" | "section" | "article" | "ul" | "li";
}) {
  const Component = motion[as];
  return (
    <Component
      className={className}
      variants={rise}
      initial="hidden"
      whileInView="shown"
      viewport={VIEWPORT}
    >
      {children}
    </Component>
  );
}

export function RevealGroup({
  children,
  className,
  as = "div",
}: {
  children: React.ReactNode;
  className?: string;
  as?: "div" | "section" | "ul" | "dl";
}) {
  const Component = motion[as];
  return (
    <Component
      className={className}
      variants={stagger}
      initial="hidden"
      whileInView="shown"
      viewport={VIEWPORT}
    >
      {children}
    </Component>
  );
}

/** A line of type that prints, rather than fades — it rises out of a clipped mask. */
export function PrintedLine({
  children,
  delay = 0,
  className,
}: {
  children: React.ReactNode;
  delay?: number;
  className?: string;
}) {
  const reduced = useReducedMotion();
  return (
    <span className="block overflow-hidden">
      <motion.span
        className={className}
        initial={reduced ? { opacity: 0 } : { y: "110%" }}
        animate={reduced ? { opacity: 1 } : { y: 0 }}
        transition={{ duration: 0.62, ease: EASE_OUT, delay }}
        style={{ display: "inline-block" }}
      >
        {children}
      </motion.span>
    </span>
  );
}
