import { useState } from "react";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import {
  BookOpenIcon,
  ChatIcon,
  ChartLineUpIcon,
  CurrencyCircleDollarIcon,
  GearSixIcon,
  PlanetIcon,
  SquaresFourIcon,
  WalletIcon,
  type Icon,
} from "@phosphor-icons/react";

type NavItem = {
  id: string;
  label: string;
  icon: Icon;
};

const MAIN_ITEMS: NavItem[] = [
  { id: "dashboard", label: "Dashboard", icon: SquaresFourIcon },
  { id: "markets", label: "Markets", icon: PlanetIcon },
  { id: "analytics", label: "Analytics", icon: ChartLineUpIcon },
  { id: "brokerage", label: "Brokerage", icon: CurrencyCircleDollarIcon },
  { id: "portfolio", label: "Portfolio", icon: WalletIcon },
];

const FOOTER_ITEMS: NavItem[] = [
  { id: "journal", label: "Journal", icon: BookOpenIcon },
  { id: "chat", label: "Chat", icon: ChatIcon },
  { id: "settings", label: "Settings", icon: GearSixIcon },
];

type SidebarButtonProps = {
  item: NavItem;
  isActive: boolean;
  onPress: () => void;
};

function SidebarButton({ item, isActive, onPress }: SidebarButtonProps) {
  const ItemIcon = item.icon;
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
          <ItemIcon size={20} weight="fill" />
        </button>
      </TooltipTrigger>
      <TooltipContent side="left">{item.label}</TooltipContent>
    </Tooltip>
  );
}

type SidebarProps = {
  active?: string;
  onActiveChange?: (id: string) => void;
};

export default function Sidebar({ active, onActiveChange }: SidebarProps) {
  const [internalActive, setInternalActive] = useState("analytics");
  const current = active ?? internalActive;

  const select = (id: string) => {
    setInternalActive(id);
    onActiveChange?.(id);
  };

  return (
    <nav
      aria-label="Primary"
      className="flex h-full w-12 shrink-0 flex-col items-center gap-4 border-l border-zinc-200/60 bg-zinc-100/60 py-4 backdrop-blur-xl dark:border-zinc-800/60 dark:bg-black/30"
    >
      {MAIN_ITEMS.map((item) => (
        <SidebarButton
          key={item.id}
          item={item}
          isActive={current === item.id}
          onPress={() => select(item.id)}
        />
      ))}
      <div className="mt-auto flex flex-col items-center gap-4">
        {FOOTER_ITEMS.map((item) => (
          <SidebarButton
            key={item.id}
            item={item}
            isActive={current === item.id}
            onPress={() => select(item.id)}
          />
        ))}
      </div>
    </nav>
  );
}
