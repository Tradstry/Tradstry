import { useState } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  AnalyticsUpIcon,
  BankIcon,
  BookOpen01Icon,
  DashboardSquare01Icon,
  File01Icon,
  Notebook01Icon,
} from "@hugeicons/core-free-icons";

type IconData = typeof DashboardSquare01Icon;

type NavItem = {
  id: string;
  label: string;
  icon: IconData;
};

const NAV_ITEMS: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: DashboardSquare01Icon },
  { id: "playbook", label: "Playbook", icon: BookOpen01Icon },
  { id: "journal", label: "Journal", icon: File01Icon },
  { id: "notebook", label: "Notebook", icon: Notebook01Icon },
  { id: "analytics", label: "Analytics", icon: AnalyticsUpIcon },
  { id: "brokerage", label: "Brokerage", icon: BankIcon },
];

export const DEFAULT_JOURNAL_PAGE = "dashboard";

/** Human label for a Journal page id (for the header title). */
export function journalPageLabel(id: string): string {
  return NAV_ITEMS.find((item) => item.id === id)?.label ?? "";
}

type SidebarButtonProps = {
  item: NavItem;
  isActive: boolean;
  onPress: () => void;
};

function SidebarButton({ item, isActive, onPress }: SidebarButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={item.label}
          onClick={onPress}
          className={`flex h-10 w-10 cursor-pointer items-center justify-center rounded-lg outline-none transition duration-150 active:scale-95 motion-reduce:transition-none motion-reduce:active:scale-100 focus-visible:outline-2 focus-visible:outline-blue-500 ${
            isActive
              ? "bg-blue-500/15 text-blue-500 dark:bg-blue-500/15 dark:text-blue-400"
              : "text-zinc-500 hover:bg-zinc-200 hover:text-zinc-900 dark:text-zinc-400 dark:hover:bg-zinc-800/70 dark:hover:text-zinc-100"
          }`}
        >
          <HugeiconsIcon icon={item.icon} size={20} strokeWidth={2} />
        </button>
      </TooltipTrigger>
      <TooltipContent side="right">{item.label}</TooltipContent>
    </Tooltip>
  );
}

type SidebarProps = {
  active?: string;
  onActiveChange?: (id: string) => void;
};

export default function JournalSidebar({ active, onActiveChange }: SidebarProps) {
  const [internalActive, setInternalActive] = useState("dashboard");
  const current = active ?? internalActive;

  const select = (id: string) => {
    setInternalActive(id);
    onActiveChange?.(id);
  };

  return (
    <nav
      aria-label="Journal"
      className="flex h-full w-12 shrink-0 flex-col items-center gap-4 border-r border-zinc-200/60 bg-zinc-100/60 py-4 backdrop-blur-xl dark:border-zinc-800/60 dark:bg-black/30"
    >
      {NAV_ITEMS.map((item) => (
        <SidebarButton
          key={item.id}
          item={item}
          isActive={current === item.id}
          onPress={() => select(item.id)}
        />
      ))}
    </nav>
  );
}
