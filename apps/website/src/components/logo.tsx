/**
 * The Tradstry mark — ruled journal lines with a trade line cutting across them.
 *
 * Strokes only, in `currentColor`, so the caller owns the ground. That keeps it in step with
 * the favicon (`app/icon.svg`, same 32-unit grid and same path data) while still inverting
 * correctly between light and dark, which a baked-in background could not do.
 */
export function TradstryMark({ className }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      className={className}
      aria-hidden="true"
    >
      <path
        d="M7 12h18M7 17h18"
        stroke="currentColor"
        strokeOpacity="0.35"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
      <path
        d="M7 23l6-6 4 3 8-11"
        stroke="currentColor"
        strokeWidth="2.6"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}
