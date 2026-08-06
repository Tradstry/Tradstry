import Link from "next/link";
import { TradstryMark } from "@tradstry/app-ui/components/logo";

const PRODUCT = [
  { href: "/#product", label: "Product" },
  { href: "/#mcp", label: "MCP" },
  { href: "/#pricing", label: "Pricing" },
  { href: "/#faq", label: "FAQ" },
];

const LEGAL = [
  { href: "/terms", label: "Terms" },
  { href: "/privacy", label: "Privacy" },
];

export function Footer() {
  return (
    <footer className="border-t border-white/[0.06] py-10">
      <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-6 px-6 sm:flex-row">
        <div className="flex items-center gap-2.5 text-zinc-300">
          <TradstryMark className="size-4" />
          <span className="text-sm font-medium">Tradstry</span>
        </div>

        <nav className="flex flex-wrap items-center justify-center gap-x-6 gap-y-2">
          {PRODUCT.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="text-sm text-zinc-500 transition-colors hover:text-zinc-200"
            >
              {link.label}
            </Link>
          ))}
          <span
            aria-hidden="true"
            className="hidden h-3 w-px bg-white/10 sm:block"
          />
          {LEGAL.map((link) => (
            <Link
              key={link.href}
              href={link.href}
              className="text-sm text-zinc-500 transition-colors hover:text-zinc-200"
            >
              {link.label}
            </Link>
          ))}
        </nav>

        <p className="text-xs text-zinc-600">
          &copy; {new Date().getFullYear()} Tradstry
        </p>
      </div>
    </footer>
  );
}
