import { useState, type ReactNode } from "react";
import {
  Button,
  Menu,
  MenuItem,
  MenuTrigger,
  Popover,
  Separator,
  ToggleButton,
  ToggleButtonGroup,
  Tooltip,
  TooltipTrigger,
} from "react-aria-components";
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
type IconButtonProps = {
  label: string;
  onPress?: () => void;
  isDisabled?: boolean;
  children: ReactNode;
};

function IconButton({ label, onPress, isDisabled, children }: IconButtonProps) {
  return (
    <TooltipTrigger delay={400}>
      <Button
        aria-label={label}
        onPress={onPress}
        isDisabled={isDisabled}
        className="flex h-7 w-7 cursor-pointer items-center justify-center rounded-md text-zinc-500 outline-none transition duration-150 data-hovered:bg-zinc-200 data-hovered:text-zinc-900 data-pressed:scale-95 data-disabled:opacity-40 data-focus-visible:outline-2 data-focus-visible:outline-blue-500 motion-reduce:transition-none dark:text-zinc-400 dark:data-hovered:bg-zinc-800 dark:data-hovered:text-zinc-100"
      >
        {children}
      </Button>
      <Tooltip
        placement="bottom"
        offset={6}
        className="rounded-md bg-zinc-800 px-2 py-1 text-xs text-zinc-100 shadow-md"
      >
        {label}
      </Tooltip>
    </TooltipTrigger>
  );
}

const segmentClass =
  "flex h-6 cursor-pointer items-center rounded-md px-3 text-sm font-medium text-zinc-500 outline-none transition duration-150 data-hovered:text-zinc-700 data-selected:bg-white data-selected:text-zinc-900 data-selected:shadow-sm data-focus-visible:outline-2 data-focus-visible:outline-blue-500 dark:text-zinc-400 dark:data-hovered:text-zinc-200 dark:data-selected:bg-zinc-700/80 dark:data-selected:text-zinc-50";

const kbdClass =
  "flex h-4.5 min-w-4.5 items-center justify-center rounded border border-zinc-300 px-1 font-sans text-[10px] text-zinc-500 dark:border-zinc-700 dark:text-zinc-400";

const menuItemClass =
  "flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-sm text-zinc-700 outline-none transition-colors data-focused:bg-zinc-100 data-focus-visible:bg-zinc-100 dark:text-zinc-200 dark:data-focused:bg-zinc-800 dark:data-focus-visible:bg-zinc-800";

const menuItemDangerClass =
  "flex cursor-pointer items-center gap-2.5 rounded-md px-2.5 py-2 text-sm text-red-600 outline-none transition-colors data-focused:bg-red-50 data-focus-visible:bg-red-50 dark:text-red-400 dark:data-focused:bg-red-950/40 dark:data-focus-visible:bg-red-950/40";

const menuSeparatorClass = "my-1 h-px bg-zinc-200 dark:bg-zinc-800";

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
        <ToggleButtonGroup
          selectionMode="single"
          disallowEmptySelection
          selectedKeys={[mode]}
          onSelectionChange={(keys) => {
            const key = [...keys][0];
            if (key) changeMode(String(key));
          }}
          className="flex items-center gap-0.5 rounded-lg bg-zinc-200/70 p-0.5 dark:bg-zinc-900"
        >
          <ToggleButton id="journal" className={segmentClass}>
            Journal
          </ToggleButton>
          <ToggleButton id="zaned" className={segmentClass}>
            Zaned
          </ToggleButton>
        </ToggleButtonGroup>
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
            <Button
              onPress={onSearchOpen}
              className="flex h-7 w-80 max-w-full cursor-pointer items-center gap-2 rounded-lg border border-zinc-200 bg-white px-2.5 text-sm text-zinc-500 outline-none transition duration-150 data-hovered:border-zinc-300 data-focus-visible:outline-2 data-focus-visible:outline-blue-500 dark:border-zinc-800 dark:bg-zinc-900 dark:text-zinc-400 dark:data-hovered:border-zinc-700"
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
            </Button>
          </>
        )}
      </div>

      <div className="flex items-center justify-end gap-2">
        {mode !== "journal" && (
          <IconButton label="Toggle right panel">
            <SquareSplitHorizontalIcon size={16} />
          </IconButton>
        )}
        <MenuTrigger>
          <Button
            aria-label="Account menu"
            className="flex cursor-pointer items-center gap-1 rounded-full py-0.5 pr-1 outline-none transition duration-150 data-hovered:bg-zinc-200 data-pressed:bg-zinc-200 data-focus-visible:outline-2 data-focus-visible:outline-blue-500 dark:data-hovered:bg-zinc-800 dark:data-pressed:bg-zinc-800"
          >
            <span className="flex h-6 w-6 items-center justify-center rounded-full bg-blue-600 text-[10px] font-semibold text-white">
              {initials}
            </span>
            <CaretDownIcon size={12} className="text-zinc-500 dark:text-zinc-400" />
          </Button>
          <Popover
            placement="bottom end"
            offset={8}
            className="w-64 rounded-xl border border-zinc-200 bg-white p-1.5 shadow-lg outline-none dark:border-zinc-800 dark:bg-zinc-900"
          >
            <div className="flex items-center gap-3 px-2.5 py-2">
              <span className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-blue-600 text-xs font-semibold text-white">
                {initials}
              </span>
              <div className="flex min-w-0 flex-col">
                <span className="truncate text-sm font-medium text-zinc-900 dark:text-zinc-100">
                  {userName ?? "Account"}
                </span>
                {userEmail && (
                  <span className="truncate text-xs text-zinc-500 dark:text-zinc-400">
                    {userEmail}
                  </span>
                )}
              </div>
            </div>
            <Separator className={menuSeparatorClass} />
            <Menu
              className="outline-none"
              onAction={(key) => {
                if (key === "logout") onSignOut?.();
              }}
            >
              <MenuItem id="security" className={menuItemClass}>
                <UserCircleIcon size={17} />
                Account &amp; Security
              </MenuItem>
              <MenuItem id="settings" className={menuItemClass}>
                <GearSixIcon size={17} />
                Settings
              </MenuItem>
              <Separator className={menuSeparatorClass} />
              <MenuItem id="logout" className={menuItemDangerClass}>
                <SignOutIcon size={17} />
                Log out
              </MenuItem>
            </Menu>
          </Popover>
        </MenuTrigger>
      </div>
    </header>
  );
}
