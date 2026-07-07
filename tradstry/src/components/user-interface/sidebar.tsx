import { useState } from "react";
import { Button, Tooltip, TooltipTrigger } from "react-aria-components";
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
    <TooltipTrigger delay={400}>
      <Button
        aria-label={item.label}
        onPress={onPress}
        className={`flex h-10 w-10 cursor-pointer items-center justify-center rounded-lg outline-none transition duration-150 data-pressed:scale-95 motion-reduce:transition-none motion-reduce:data-pressed:scale-100 data-focus-visible:outline-2 data-focus-visible:outline-blue-500 ${
          isActive
            ? "bg-blue-500/15 text-blue-500 dark:bg-blue-500/15 dark:text-blue-400"
            : "text-zinc-500 data-hovered:bg-zinc-200 data-hovered:text-zinc-900 dark:text-zinc-400 dark:data-hovered:bg-zinc-800/70 dark:data-hovered:text-zinc-100"
        }`}
      >
        <ItemIcon size={20} weight="fill" />
      </Button>
      <Tooltip
        placement="left"
        offset={8}
        className="rounded-md bg-zinc-800 px-2 py-1 text-xs text-zinc-100 shadow-md dark:bg-zinc-800"
      >
        {item.label}
      </Tooltip>
    </TooltipTrigger>
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
