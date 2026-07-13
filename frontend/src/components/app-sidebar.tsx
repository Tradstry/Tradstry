"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import {
  AnalyticsUpIcon,
  BankIcon,
  BookOpen01Icon,
  Calculator01Icon,
  CommandIcon,
  DashboardSquare01Icon,
  File01Icon,
  Notebook01Icon,
} from "@hugeicons/core-free-icons";
import Link from "next/link";
import { usePathname } from "next/navigation";
import * as React from "react";
import { NavUser } from "@/components/nav-user";
import { PositionCalculator } from "@/components/position-calculator";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

type IconData = typeof DashboardSquare01Icon;

type NavItem = { title: string; url: string; icon: IconData };

const NAV_ITEMS: NavItem[] = [
  { title: "Dashboard", url: "/dashboard", icon: DashboardSquare01Icon },
  { title: "Playbook", url: "/dashboard/playbook", icon: BookOpen01Icon },
  { title: "Journal", url: "/dashboard/journal", icon: File01Icon },
  { title: "Notebook", url: "/dashboard/notebook", icon: Notebook01Icon },
  { title: "Analytics", url: "/dashboard/analytics", icon: AnalyticsUpIcon },
  { title: "Brokerage", url: "/dashboard/brokerage", icon: BankIcon },
];

/** Highlight the deepest matching route so nested paths keep their parent active. */
function isActive(pathname: string, url: string): boolean {
  if (url === "/dashboard") return pathname === "/dashboard";
  return pathname === url || pathname.startsWith(`${url}/`);
}

export function AppSidebar() {
  const pathname = usePathname();
  const [calculatorOpen, setCalculatorOpen] = React.useState(false);

  return (
    <TooltipProvider delayDuration={200}>
      <aside
        aria-label="Primary"
        className="flex h-svh w-14 shrink-0 flex-col items-center gap-1 border-r border-border/60 bg-muted/30 py-3 backdrop-blur-xl"
      >
        <Link
          href="/dashboard"
          aria-label="Tradstry home"
          className="mb-2 flex size-9 items-center justify-center rounded-lg bg-foreground text-background outline-none transition-transform duration-150 hover:scale-105 focus-visible:ring-2 focus-visible:ring-blue-500/70 active:scale-95"
        >
          <HugeiconsIcon icon={CommandIcon} size={18} strokeWidth={2.5} />
        </Link>

        <nav className="flex flex-col items-center gap-1" aria-label="Journal">
          {NAV_ITEMS.map((item) => {
            const active = isActive(pathname, item.url);
            return (
              <Tooltip key={item.url}>
                <TooltipTrigger asChild>
                  <Link
                    href={item.url}
                    aria-label={item.title}
                    aria-current={active ? "page" : undefined}
                    className={cn(
                      "flex size-11 items-center justify-center rounded-xl outline-none transition-colors duration-150",
                      "focus-visible:ring-2 focus-visible:ring-blue-500/70",
                      active
                        ? "bg-blue-500/15 text-blue-600 dark:text-blue-400"
                        : "text-muted-foreground hover:bg-muted hover:text-foreground",
                    )}
                  >
                    <HugeiconsIcon icon={item.icon} size={20} strokeWidth={2} />
                  </Link>
                </TooltipTrigger>
                <TooltipContent side="right">{item.title}</TooltipContent>
              </Tooltip>
            );
          })}
        </nav>

        <div className="mt-auto flex flex-col items-center gap-1">
          <Tooltip>
            <TooltipTrigger asChild>
              <button
                type="button"
                aria-label="Position Calculator"
                onClick={() => setCalculatorOpen(true)}
                className="flex size-11 items-center justify-center rounded-xl text-muted-foreground outline-none transition-colors duration-150 hover:bg-muted hover:text-foreground focus-visible:ring-2 focus-visible:ring-blue-500/70"
              >
                <HugeiconsIcon icon={Calculator01Icon} size={20} strokeWidth={2} />
              </button>
            </TooltipTrigger>
            <TooltipContent side="right">Position Calculator</TooltipContent>
          </Tooltip>
          <NavUser />
        </div>
      </aside>

      <PositionCalculator
        open={calculatorOpen}
        onOpenChange={setCalculatorOpen}
      />
    </TooltipProvider>
  );
}
