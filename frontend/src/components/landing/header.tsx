"use client";

import { SignInButton, SignUpButton } from "@clerk/nextjs";
import { Button } from "@/components/ui/button";

export function Header() {
  return (
    <header className="sticky top-0 z-50 w-full border-b border-white/5 bg-black/80 backdrop-blur-md">
      <div className="mx-auto flex h-14 max-w-6xl items-center justify-between px-6">
        <span className="text-lg font-bold tracking-tight text-white">
          Tradstry
        </span>

        <nav className="hidden items-center gap-6 md:flex">
          <a
            href="#features"
            className="text-sm text-zinc-400 transition-colors hover:text-white"
          >
            Features
          </a>
          <a
            href="#pricing"
            className="text-sm text-zinc-400 transition-colors hover:text-white"
          >
            Pricing
          </a>
        </nav>

        <div className="flex items-center gap-3">
          <SignInButton>
            <Button variant="ghost" size="lg" className="text-zinc-300 hover:text-white hover:bg-white/10">
              Sign In
            </Button>
          </SignInButton>
          <SignUpButton>
            <Button size="lg" className="bg-white text-black hover:bg-zinc-200">
              Get Started
            </Button>
          </SignUpButton>
        </div>
      </div>
    </header>
  );
}
