"use client";

import type * as React from "react";
import { PLACEHOLDER } from "@/components/landing/content";
import { cn } from "@/lib/utils";

export function Section({
  id,
  children,
  className,
}: {
  id?: string;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <section
      id={id}
      className={cn("border-t border-white/[0.06] py-24 md:py-32", className)}
    >
      <div className="mx-auto max-w-6xl px-6">{children}</div>
    </section>
  );
}

export function Eyebrow({ children }: { children: React.ReactNode }) {
  return (
    <p className="flex items-center gap-3 text-[11px] font-medium uppercase tracking-[0.18em] text-zinc-400">
      <span aria-hidden="true" className="h-px w-6 bg-white/25" />
      {children}
    </p>
  );
}

export function Heading({
  children,
  className,
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <h2
      className={cn(
        "mt-3 text-balance text-3xl font-semibold tracking-[-0.02em] text-zinc-50 md:text-[2.75rem] md:leading-[1.1]",
        className,
      )}
    >
      {children}
    </h2>
  );
}

export function Lede({ children }: { children: React.ReactNode }) {
  return (
    <p className="mt-4 max-w-xl text-[15px] leading-relaxed text-zinc-400">
      {children}
    </p>
  );
}

/** Ruled paper, straight off the mark — the one motif the whole page is built on. */
export function Ruled({ className }: { className?: string }) {
  return (
    <div
      aria-hidden="true"
      className={cn(
        "pointer-events-none bg-[repeating-linear-gradient(to_bottom,transparent_0,transparent_23px,rgba(255,255,255,0.05)_23px,rgba(255,255,255,0.05)_24px)]",
        className,
      )}
    />
  );
}

export function Shot({
  shot,
  className,
}: {
  shot: { src: string | null; alt: string; ratio: string };
  className?: string;
}) {
  return (
    <figure
      className={cn(
        "overflow-hidden rounded-xl border border-white/10 bg-[#131316] shadow-2xl shadow-black/60",
        className,
      )}
    >
      <div className="flex items-center gap-1.5 border-b border-white/[0.06] px-3 py-2.5">
        <span className="size-2 rounded-full bg-white/10" />
        <span className="size-2 rounded-full bg-white/10" />
        <span className="size-2 rounded-full bg-white/10" />
      </div>
      {shot.src ? (
        // biome-ignore lint/performance/noImgElement: static marketing asset, no layout shift risk at a fixed ratio
        <img
          src={shot.src}
          alt={shot.alt}
          style={{ aspectRatio: shot.ratio }}
          className="w-full object-cover"
        />
      ) : (
        <div
          style={{ aspectRatio: shot.ratio }}
          className="relative grid w-full place-items-center"
        >
          <Ruled className="absolute inset-0" />
          <p className="relative font-mono text-xs text-zinc-600">{shot.alt}</p>
        </div>
      )}
    </figure>
  );
}

export function Pending({ children }: { children: string }) {
  if (children !== PLACEHOLDER) return <>{children}</>;
  return (
    <span className="rounded border border-white/15 bg-white/[0.06] px-1.5 py-0.5 font-mono text-[0.8em] text-zinc-400">
      TODO
    </span>
  );
}
