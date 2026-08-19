"use client";

import { SignInButton, SignUpButton } from "@clerk/nextjs";
import { TradstryMark } from "@tradstry/app-ui/components/logo";
import { Button } from "@tradstry/app-ui/components/ui/button";

const NAV = [
  { href: "/#leak", label: "The leak" },
  { href: "/#product", label: "Product" },
  { href: "/#mcp", label: "MCP" },
  { href: "/#faq", label: "FAQ" },
];

export function Header() {
  return (
    <header className="sticky top-0 z-50 w-full border-b border-white/[0.06] bg-[#0A0A0B]/80 backdrop-blur-xl">
      <div className="mx-auto flex h-16 max-w-6xl items-center justify-between px-6">
        <a href="/" className="flex items-center gap-2.5 text-zinc-50">
          <span className="flex size-8 items-center justify-center rounded-lg bg-zinc-50 text-[#0A0A0B]">
            <TradstryMark className="size-[18px]" />
          </span>
          <span className="text-[15px] font-semibold tracking-tight">
            Tradstry
          </span>
        </a>

        <nav className="hidden items-center gap-8 md:flex">
          {NAV.map((item) => (
            <a
              key={item.href}
              href={item.href}
              className="relative text-sm text-zinc-400 transition-colors after:absolute after:inset-x-0 after:-bottom-1.5 after:h-px after:origin-left after:scale-x-0 after:bg-zinc-50 after:transition-transform after:duration-200 after:ease-out hover:text-zinc-50 hover:after:scale-x-100"
            >
              {item.label}
            </a>
          ))}
        </nav>

        <div className="flex items-center gap-2">
          <SignInButton>
            <Button
              variant="ghost"
              className="text-zinc-300 hover:bg-white/5 hover:text-zinc-50"
            >
              Sign in
            </Button>
          </SignInButton>
          <SignUpButton>
            <Button className="bg-zinc-50 text-[#0A0A0B] transition-transform duration-150 hover:bg-zinc-200 active:scale-[0.97]">
              Start journalling
            </Button>
          </SignUpButton>
        </div>
      </div>
    </header>
  );
}
