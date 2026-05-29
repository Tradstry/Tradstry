"use client";

import { SignInButton, SignUpButton } from "@clerk/nextjs";
import {
  AiBrain01Icon,
  AiChat02Icon,
  Analytics02Icon,
  BankIcon,
  ChartLineData02Icon,
  ConnectIcon,
  Edit02Icon,
  LinkSquare01Icon,
  News01Icon,
  PencilEdit02Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { Button } from "@/components/ui/button";

// ── Mockups ──────────────────────────────────────────────

function DashboardMockup() {
  return (
    <div className="relative rounded-xl border border-white/10 bg-zinc-900/80 p-4 shadow-2xl shadow-blue-500/5">
      <div className="mb-4 grid grid-cols-3 gap-3">
        <div className="rounded-lg bg-zinc-800/80 p-3">
          <div className="text-xs text-zinc-500">Total P&L</div>
          <div className="text-lg font-semibold text-emerald-400">+$12,847</div>
        </div>
        <div className="rounded-lg bg-zinc-800/80 p-3">
          <div className="text-xs text-zinc-500">Win Rate</div>
          <div className="text-lg font-semibold text-white">68.4%</div>
        </div>
        <div className="rounded-lg bg-zinc-800/80 p-3">
          <div className="text-xs text-zinc-500">Trades</div>
          <div className="text-lg font-semibold text-white">142</div>
        </div>
      </div>
      <div className="relative h-32 overflow-hidden rounded-lg bg-zinc-800/50">
        <svg
          viewBox="0 0 400 120"
          className="h-full w-full"
          preserveAspectRatio="none"
        >
          <defs>
            <linearGradient id="chartGrad" x1="0" y1="0" x2="0" y2="1">
              <stop
                offset="0%"
                stopColor="oklch(0.546 0.245 262.881)"
                stopOpacity="0.3"
              />
              <stop
                offset="100%"
                stopColor="oklch(0.546 0.245 262.881)"
                stopOpacity="0"
              />
            </linearGradient>
          </defs>
          <path
            d="M0,80 Q50,70 100,60 T200,40 T300,50 T400,20"
            fill="none"
            stroke="oklch(0.546 0.245 262.881)"
            strokeWidth="2"
          />
          <path
            d="M0,80 Q50,70 100,60 T200,40 T300,50 T400,20 L400,120 L0,120 Z"
            fill="url(#chartGrad)"
          />
        </svg>
      </div>
      <div className="mt-4 space-y-2">
        <div className="flex items-center justify-between rounded-md bg-zinc-800/50 px-3 py-2">
          <span className="text-xs text-zinc-400">AAPL</span>
          <span className="text-xs font-medium text-emerald-400">+$340</span>
        </div>
        <div className="flex items-center justify-between rounded-md bg-zinc-800/50 px-3 py-2">
          <span className="text-xs text-zinc-400">TSLA</span>
          <span className="text-xs font-medium text-red-400">-$120</span>
        </div>
        <div className="flex items-center justify-between rounded-md bg-zinc-800/50 px-3 py-2">
          <span className="text-xs text-zinc-400">NVDA</span>
          <span className="text-xs font-medium text-emerald-400">+$890</span>
        </div>
      </div>
    </div>
  );
}

function JournalMockup() {
  return (
    <div className="rounded-xl border border-white/10 bg-zinc-900/80 p-4 shadow-2xl shadow-blue-500/5">
      <div className="mb-3 flex items-center gap-2">
        <div className="h-2 w-2 rounded-full bg-emerald-400" />
        <span className="text-xs font-medium text-zinc-400">
          AAPL Earnings Play — Mar 28
        </span>
      </div>
      <div className="space-y-2 rounded-lg bg-zinc-800/50 p-3">
        <div className="h-3 w-3/4 rounded bg-zinc-700/50" />
        <div className="h-3 w-full rounded bg-zinc-700/50" />
        <div className="h-3 w-5/6 rounded bg-zinc-700/50" />
        <div className="h-3 w-2/3 rounded bg-zinc-700/50" />
      </div>
      <div className="mt-3 flex gap-2">
        <span className="rounded-full bg-blue-500/10 px-2 py-0.5 text-[10px] text-blue-400">
          earnings
        </span>
        <span className="rounded-full bg-purple-500/10 px-2 py-0.5 text-[10px] text-purple-400">
          swing
        </span>
        <span className="rounded-full bg-emerald-500/10 px-2 py-0.5 text-[10px] text-emerald-400">
          winner
        </span>
      </div>
    </div>
  );
}

function AnalyticsMockup() {
  return (
    <div className="rounded-xl border border-white/10 bg-zinc-900/80 p-4 shadow-2xl shadow-blue-500/5">
      <div className="mb-4 grid grid-cols-2 gap-3">
        <div className="rounded-lg bg-zinc-800/80 p-3">
          <div className="text-xs text-zinc-500">Sharpe Ratio</div>
          <div className="text-lg font-semibold text-white">1.84</div>
        </div>
        <div className="rounded-lg bg-zinc-800/80 p-3">
          <div className="text-xs text-zinc-500">Max Drawdown</div>
          <div className="text-lg font-semibold text-red-400">-8.2%</div>
        </div>
      </div>
      <div className="relative h-28 overflow-hidden rounded-lg bg-zinc-800/50">
        <svg
          viewBox="0 0 400 100"
          className="h-full w-full"
          preserveAspectRatio="none"
        >
          <rect
            x="20"
            y="60"
            width="30"
            height="40"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.6"
          />
          <rect
            x="65"
            y="40"
            width="30"
            height="60"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.7"
          />
          <rect
            x="110"
            y="25"
            width="30"
            height="75"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.8"
          />
          <rect
            x="155"
            y="45"
            width="30"
            height="55"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.6"
          />
          <rect
            x="200"
            y="15"
            width="30"
            height="85"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.9"
          />
          <rect
            x="245"
            y="30"
            width="30"
            height="70"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.75"
          />
          <rect
            x="290"
            y="20"
            width="30"
            height="80"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
            opacity="0.85"
          />
          <rect
            x="335"
            y="10"
            width="30"
            height="90"
            rx="2"
            fill="oklch(0.546 0.245 262.881)"
          />
        </svg>
      </div>
    </div>
  );
}

const MCP_TOOLS = [
  "list_accounts",
  "query_trades",
  "calculate_analytics",
  "search_trades",
  "get_playbook_stats",
];

function McpMockup() {
  return (
    <div className="rounded-xl border border-white/10 bg-zinc-900/80 p-4 shadow-2xl shadow-blue-500/5">
      <div className="mb-3 flex items-center gap-2">
        <div className="h-2 w-2 rounded-full bg-emerald-400" />
        <span className="text-xs font-medium text-zinc-400">
          Tradstry MCP · Connected
        </span>
      </div>
      <div className="rounded-lg bg-zinc-800/50 p-3 font-mono text-[11px] leading-relaxed text-zinc-400">
        <div className="text-zinc-500">{"// claude config"}</div>
        <div>
          <span className="text-zinc-300">"tradstry"</span>: {"{"}
        </div>
        <div className="pl-3">
          <span className="text-zinc-300">"url"</span>:{" "}
          <span className="text-emerald-400">"https://mcp.tradstry.com"</span>
        </div>
        <div>{"}"}</div>
      </div>
      <div className="mt-3 text-[10px] uppercase tracking-wider text-zinc-500">
        Available tools
      </div>
      <div className="mt-2 flex flex-wrap gap-1.5">
        {MCP_TOOLS.map((tool) => (
          <span
            key={tool}
            className="rounded-md bg-blue-500/10 px-2 py-0.5 font-mono text-[10px] text-blue-300"
          >
            {tool}
          </span>
        ))}
      </div>
      <div className="mt-3 flex items-center gap-1.5 text-[10px] text-zinc-500">
        <svg
          className="size-3 text-emerald-400"
          viewBox="0 0 16 16"
          fill="none"
        >
          <path
            d="M3 8.5l3.5 3.5L13 4.5"
            stroke="currentColor"
            strokeWidth="1.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </svg>
        Read-only access · OAuth secured
      </div>
    </div>
  );
}

// ── Sections ─────────────────────────────────────────────

function HeroSection() {
  return (
    <section className="relative overflow-hidden py-24 md:py-32">
      <div className="pointer-events-none absolute top-1/2 right-0 -translate-y-1/2 h-[600px] w-[600px] rounded-full bg-blue-500/10 blur-[120px]" />
      <div className="relative mx-auto grid max-w-6xl items-center gap-12 px-6 md:grid-cols-2">
        <div>
          <h1 className="text-4xl font-bold tracking-tight text-white md:text-5xl lg:text-6xl">
            Trade smarter with AI-powered insights
          </h1>
          <p className="mt-6 max-w-lg text-lg text-zinc-400">
            The trading journal that learns from your trades. Track performance,
            journal your decisions, and let AI uncover patterns you'd miss.
          </p>
          <div className="mt-8 flex gap-4">
            <SignUpButton>
              <Button
                size="lg"
                className="h-11 px-6 bg-white text-black hover:bg-zinc-200 text-sm"
              >
                Get Started Free
              </Button>
            </SignUpButton>
            <SignInButton>
              <Button
                variant="outline"
                size="lg"
                className="h-11 px-6 border-white/20 text-zinc-300 hover:bg-white/10 hover:text-white text-sm"
              >
                Sign In
              </Button>
            </SignInButton>
          </div>
        </div>
        <div className="relative">
          <DashboardMockup />
        </div>
      </div>
    </section>
  );
}

const features = [
  {
    icon: Analytics02Icon,
    title: "AI-Powered Insights",
    description:
      "Automated behavioral analysis, pattern recognition, and personalized recommendations to sharpen your edge.",
  },
  {
    icon: Edit02Icon,
    title: "Advanced Journaling",
    description:
      "Rich text editor with trade tagging, playbooks, and multimedia support to capture every decision.",
  },
  {
    icon: ChartLineData02Icon,
    title: "Real-time Analytics",
    description:
      "Performance tracking with risk metrics, P&L analysis, and market correlation insights.",
  },
  {
    icon: BankIcon,
    title: "Brokerage Integration",
    description:
      "Connect your accounts for automatic trade importing and real-time position tracking.",
  },
  {
    icon: News01Icon,
    title: "Market Data",
    description:
      "Live quotes, historical data, technical indicators, and curated news aggregation.",
  },
  {
    icon: AiChat02Icon,
    title: "AI Chat",
    description:
      "Interactive AI assistant for trading analysis, strategy discussions, and on-demand reports.",
  },
  {
    icon: ConnectIcon,
    title: "Connect to Claude (MCP)",
    description:
      "Query your journal from Claude or any MCP client — read-only and OAuth-secured. Your AI works straight from your real trades.",
  },
];

function FeaturesSection() {
  return (
    <section id="features" className="py-24">
      <div className="mx-auto max-w-6xl px-6">
        <h2 className="text-center text-3xl font-bold tracking-tight text-white md:text-4xl">
          Everything you need to trade better
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-zinc-400">
          A complete toolkit for tracking, analyzing, and improving your trading
          performance.
        </p>
        <div className="mt-16 grid gap-6 md:grid-cols-2 lg:grid-cols-3">
          {features.map((feature) => (
            <div
              key={feature.title}
              className="rounded-xl border border-white/5 bg-zinc-900/50 p-6 transition-colors hover:border-white/10"
            >
              <div className="flex size-10 items-center justify-center rounded-lg bg-white/5">
                <HugeiconsIcon
                  icon={feature.icon}
                  className="size-5 text-zinc-300"
                />
              </div>
              <h3 className="mt-4 text-base font-semibold text-white">
                {feature.title}
              </h3>
              <p className="mt-2 text-sm text-zinc-400">
                {feature.description}
              </p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

const showcases = [
  {
    title: "Your trading command center",
    description:
      "See your entire portfolio at a glance. Real-time P&L, open positions, and market overview — all in one place.",
    mockup: "dashboard",
  },
  {
    title: "Journal every decision",
    description:
      "Rich text editor with trade context, custom tags, and AI-powered search. Never lose the reasoning behind a trade.",
    mockup: "journal",
  },
  {
    title: "Analytics that find your edge",
    description:
      "Deep performance analysis with risk metrics, behavioral patterns, and AI-generated insights to refine your strategy.",
    mockup: "analytics",
  },
  {
    title: "Bring your journal into any AI assistant",
    description:
      "Connect Tradstry to Claude through the Model Context Protocol. Ask questions in plain English and your AI pulls real answers — querying trades, running analytics, and searching your journal. Read-only and OAuth-secured, so your data stays safe.",
    mockup: "mcp",
  },
];

function ShowcaseSection() {
  const mockups: Record<string, React.ReactNode> = {
    dashboard: <DashboardMockup />,
    journal: <JournalMockup />,
    analytics: <AnalyticsMockup />,
    mcp: <McpMockup />,
  };

  return (
    <section className="py-24">
      <div className="mx-auto max-w-6xl space-y-24 px-6">
        {showcases.map((item, i) => (
          <div
            key={item.title}
            className="grid items-center gap-12 md:grid-cols-2"
          >
            <div className={i % 2 === 1 ? "md:order-2" : ""}>
              <h3 className="text-2xl font-bold tracking-tight text-white md:text-3xl">
                {item.title}
              </h3>
              <p className="mt-4 text-zinc-400">{item.description}</p>
            </div>
            <div className={i % 2 === 1 ? "md:order-1" : ""}>
              {mockups[item.mockup]}
            </div>
          </div>
        ))}
      </div>
    </section>
  );
}

const steps = [
  {
    number: "1",
    icon: LinkSquare01Icon,
    title: "Connect your brokerage",
    description:
      "Link your trading accounts in seconds. We import your trades automatically.",
  },
  {
    number: "2",
    icon: PencilEdit02Icon,
    title: "Journal your trades",
    description:
      "Add context, tags, and notes to every trade. Build your personal playbook.",
  },
  {
    number: "3",
    icon: AiBrain01Icon,
    title: "Get AI insights",
    description:
      "Our AI analyzes your patterns and delivers personalized recommendations.",
  },
];

function HowItWorksSection() {
  return (
    <section className="py-24">
      <div className="mx-auto max-w-6xl px-6">
        <h2 className="text-center text-3xl font-bold tracking-tight text-white md:text-4xl">
          How it works
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-zinc-400">
          Get started in three simple steps.
        </p>
        <div className="relative mt-16 grid gap-8 md:grid-cols-3">
          <div className="pointer-events-none absolute top-8 left-[16.67%] hidden h-px w-2/3 bg-gradient-to-r from-white/10 via-white/20 to-white/10 md:block" />
          {steps.map((step) => (
            <div key={step.number} className="relative text-center">
              <div className="mx-auto flex size-16 items-center justify-center rounded-full border border-white/10 bg-zinc-900">
                <HugeiconsIcon
                  icon={step.icon}
                  className="size-6 text-zinc-300"
                />
              </div>
              <span className="mt-1 inline-block text-xs font-medium text-zinc-500">
                Step {step.number}
              </span>
              <h3 className="mt-2 text-base font-semibold text-white">
                {step.title}
              </h3>
              <p className="mt-2 text-sm text-zinc-400">{step.description}</p>
            </div>
          ))}
        </div>
      </div>
    </section>
  );
}

const pricingFeatures = [
  "Unlimited trade journaling",
  "AI-powered insights & chat",
  "Real-time analytics dashboard",
  "Brokerage account sync",
  "Market data & news",
  "Custom tags & playbooks",
  "Connect to Claude via MCP",
];

function PricingSection() {
  return (
    <section id="pricing" className="py-24">
      <div className="mx-auto max-w-6xl px-6">
        <h2 className="text-center text-3xl font-bold tracking-tight text-white md:text-4xl">
          Simple pricing
        </h2>
        <p className="mx-auto mt-4 max-w-2xl text-center text-zinc-400">
          Everything you need, completely free.
        </p>
        <div className="mx-auto mt-16 max-w-md rounded-2xl border border-white/10 bg-zinc-900/50 p-8">
          <div className="text-center">
            <h3 className="text-xl font-bold text-white">Free</h3>
            <p className="mt-1 text-sm text-zinc-400">
              No credit card required
            </p>
          </div>
          <ul className="mt-8 space-y-3">
            {pricingFeatures.map((feature) => (
              <li
                key={feature}
                className="flex items-center gap-3 text-sm text-zinc-300"
              >
                <svg
                  className="size-4 shrink-0 text-emerald-400"
                  viewBox="0 0 16 16"
                  fill="none"
                >
                  <path
                    d="M3 8.5l3.5 3.5L13 4.5"
                    stroke="currentColor"
                    strokeWidth="1.5"
                    strokeLinecap="round"
                    strokeLinejoin="round"
                  />
                </svg>
                {feature}
              </li>
            ))}
          </ul>
          <SignUpButton>
            <Button
              size="lg"
              className="mt-8 h-11 w-full bg-white text-black hover:bg-zinc-200 text-sm"
            >
              Get Started Free
            </Button>
          </SignUpButton>
        </div>
      </div>
    </section>
  );
}

function CTASection() {
  return (
    <section className="relative overflow-hidden py-24">
      <div className="pointer-events-none absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 h-[400px] w-[600px] rounded-full bg-blue-500/10 blur-[120px]" />
      <div className="relative mx-auto max-w-6xl px-6 text-center">
        <h2 className="text-3xl font-bold tracking-tight text-white md:text-4xl">
          Start improving your trading today
        </h2>
        <p className="mx-auto mt-4 max-w-lg text-zinc-400">
          Join traders who use AI-powered insights to make better decisions.
          Free to get started, no credit card required.
        </p>
        <SignUpButton>
          <Button
            size="lg"
            className="mt-8 h-11 px-8 bg-white text-black hover:bg-zinc-200 text-sm"
          >
            Get Started Free
          </Button>
        </SignUpButton>
      </div>
    </section>
  );
}

// ── Export ────────────────────────────────────────────────

export function Body() {
  return (
    <main>
      <HeroSection />
      <FeaturesSection />
      <ShowcaseSection />
      <HowItWorksSection />
      <PricingSection />
      <CTASection />
    </main>
  );
}
