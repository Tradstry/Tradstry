import * as ScrollAreaPrimitive from "@radix-ui/react-scroll-area";
import type { ComponentPropsWithoutRef } from "react";

type ScrollAreaProps = ComponentPropsWithoutRef<
  typeof ScrollAreaPrimitive.Root
>;

/**
 * Custom scroll area with a thin, themed, auto-hiding scrollbar — used instead
 * of the browser's native scrollbar. Give it a bounded height (e.g. `h-full`
 * inside a `flex-1 min-h-0` parent) and it scrolls its children.
 */
export function ScrollArea({ className, children, ...props }: ScrollAreaProps) {
  return (
    <ScrollAreaPrimitive.Root
      type="hover"
      scrollHideDelay={400}
      className={`relative overflow-hidden ${className ?? ""}`}
      {...props}
    >
      <ScrollAreaPrimitive.Viewport className="h-full w-full overscroll-contain rounded-[inherit] focus-visible:outline-none">
        {children}
      </ScrollAreaPrimitive.Viewport>
      <ScrollBar orientation="vertical" />
      <ScrollBar orientation="horizontal" />
      <ScrollAreaPrimitive.Corner />
    </ScrollAreaPrimitive.Root>
  );
}

function ScrollBar({
  orientation = "vertical",
}: {
  orientation?: "vertical" | "horizontal";
}) {
  const base =
    "flex touch-none select-none p-0.5 transition-opacity duration-150 data-[state=hidden]:opacity-0";
  const dir =
    orientation === "vertical"
      ? "h-full w-2.5 border-l border-l-transparent"
      : "h-2.5 flex-col border-t border-t-transparent";
  return (
    <ScrollAreaPrimitive.Scrollbar
      orientation={orientation}
      className={`${base} ${dir}`}
    >
      <ScrollAreaPrimitive.Thumb className="relative flex-1 rounded-full bg-zinc-300/80 transition-colors hover:bg-zinc-400 dark:bg-zinc-700/70 dark:hover:bg-zinc-600" />
    </ScrollAreaPrimitive.Scrollbar>
  );
}
