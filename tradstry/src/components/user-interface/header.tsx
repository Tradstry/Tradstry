import { useState, type ReactNode } from "react";
import {
  ArrowLeftIcon,
  ArrowRightIcon,
  CaretDownIcon,
  GearSixIcon,
  MagnifyingGlassIcon,
  SidebarSimpleIcon,
  SignOutIcon,
  SquareSplitHorizontalIcon,
  UserCircleIcon,
} from "@phosphor-icons/react";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

type IconButtonProps = {
  label: string;
  onPress?: () => void;
  isDisabled?: boolean;
  children: ReactNode;
};

function IconButton({ label, onPress, isDisabled, children }: IconButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          aria-label={label}
          onClick={onPress}
          disabled={isDisabled}
          className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-zinc-500 outline-none transition duration-150 hover:bg-zinc-200 hover:text-zinc-900 active:scale-95 disabled:pointer-events-none disabled:opacity-40 focus-visible:outline-2 focus-visible:outline-blue-500 motion-reduce:transition-none dark:text-zinc-400 dark:hover:bg-zinc-800 dark:hover:text-zinc-100"
        >
          {children}
        </button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}

const kbdClass =
  "flex h-4.5 min-w-4.5 items-center justify-center rounded border border-zinc-300 px-1 font-sans text-[10px] text-zinc-500 dark:border-zinc-700 dark:text-zinc-400";

const MODES: [string, string][] = [
  ["journal", "Journal"],
  ["zaned", "Zaned"],
];

type HeaderProps = {
  mode?: string;
  onModeChange?: (mode: string) => void;
  onSearchOpen?: () => void;
  userName?: string | null;
  userEmail?: string | null;
  onSignOut?: () => void;
};

function computeInitials(name?: string | null, email?: string | null): string {
  const parts = (name ?? "").trim().split(/\s+/).filter(Boolean);
  if (parts.length >= 2) {
    return (parts[0][0] + parts[parts.length - 1][0]).toUpperCase();
  }
  if (parts.length === 1) {
    return parts[0][0].toUpperCase();
  }
  return email?.[0]?.toUpperCase() ?? "?";
}

export default function Header({
  mode: modeProp,
  onModeChange,
  onSearchOpen,
  userName,
  userEmail,
  onSignOut,
}: HeaderProps) {
  const [internalMode, setInternalMode] = useState("zaned");
  const mode = modeProp ?? internalMode;

  const initials = computeInitials(userName, userEmail);

  const changeMode = (next: string) => {
    setInternalMode(next);
    onModeChange?.(next);
  };

  return (
    <header className="grid h-11 shrink-0 grid-cols-[1fr_auto_1fr] items-center gap-4 border-b border-zinc-200/60 bg-zinc-50/60 px-3 backdrop-blur-xl dark:border-zinc-800/60 dark:bg-zinc-950/40">
      <div className="flex items-center gap-2">
        <div className="flex items-center gap-0.5 rounded-lg bg-zinc-200/70 p-0.5 dark:bg-zinc-900">
          {MODES.map(([id, label]) => (
            <button
              key={id}
              type="button"
              onClick={() => changeMode(id)}
              className={`flex h-6 cursor-pointer items-center rounded-md px-3 text-sm font-medium outline-none transition duration-150 focus-visible:outline-2 focus-visible:outline-blue-500 ${
                mode === id
                  ? "bg-white text-zinc-900 shadow-sm dark:bg-zinc-700/80 dark:text-zinc-50"
                  : "text-zinc-500 hover:text-zinc-700 dark:text-zinc-400 dark:hover:text-zinc-200"
              }`}
            >
              {label}
            </button>
          ))}
        </div>
        {mode !== "journal" && (
          <>
            <IconButton label="Search" onPress={onSearchOpen}>
              <MagnifyingGlassIcon size={16} />
            </IconButton>
            <IconButton label="Toggle panel">
              <SidebarSimpleIcon size={16} />
            </IconButton>
          </>
        )}
      </div>

      <div className="ml-6 flex items-center gap-1">
        {mode !== "journal" && (
          <>
            <IconButton label="Back" isDisabled>
              <ArrowLeftIcon size={16} />
            </IconButton>
            <IconButton label="Forward" isDisabled>
              <ArrowRightIcon size={16} />
            </IconButton>
            <button
              type="button"
              onClick={onSearchOpen}
              className="flex h-7 w-80 max-w-full cursor-pointer items-center gap-2 rounded-lg border border-zinc-200 bg-white px-2.5 text-sm text-zinc-500 outline-none transition duration-150 hover:border-zinc-300 focus-visible:outline-2 focus-visible:outline-blue-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-400 dark:hover:border-zinc-700"
            >
              <MagnifyingGlassIcon
                size={14}
                className="shrink-0 text-zinc-400 dark:text-zinc-500"
              />
              <span className="flex-1 truncate text-left">
                Search trades, symbols…
              </span>
              <span className="flex items-center gap-0.5">
                <kbd className={kbdClass}>⌘</kbd>
                <kbd className={kbdClass}>K</kbd>
              </span>
            </button>
          </>
        )}
      </div>

      <div className="flex items-center justify-end gap-2">
        {mode !== "journal" && (
          <IconButton label="Toggle right panel">
            <SquareSplitHorizontalIcon size={16} />
          </IconButton>
        )}
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              aria-label="Account menu"
              className="flex cursor-pointer items-center gap-1 rounded-full py-0.5 pr-1 outline-none transition duration-150 hover:bg-zinc-200 focus-visible:outline-2 focus-visible:outline-blue-500 dark:hover:bg-zinc-800"
            >
              <span className="flex h-6 w-6 items-center justify-center rounded-full bg-blue-600 text-[10px] font-semibold text-white">
                {initials}
              </span>
              <CaretDownIcon
                size={12}
                className="text-zinc-500 dark:text-zinc-400"
              />
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" sideOffset={8} className="w-64">
            <div className="flex items-center gap-3 px-2 py-1.5">
              <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-blue-600 text-xs font-semibold text-white">
                {initials}
              </span>
              <div className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium text-foreground">
                  {userName ?? "Account"}
                </span>
                {userEmail && (
                  <span className="truncate text-xs text-muted-foreground">
                    {userEmail}
                  </span>
                )}
              </div>
            </div>
            <DropdownMenuSeparator />
            <DropdownMenuItem>
              <UserCircleIcon size={17} />
              Account &amp; Security
            </DropdownMenuItem>
            <DropdownMenuItem>
              <GearSixIcon size={17} />
              Settings
            </DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem
              variant="destructive"
              onSelect={() => onSignOut?.()}
            >
              <SignOutIcon size={17} />
              Log out
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  );
}
