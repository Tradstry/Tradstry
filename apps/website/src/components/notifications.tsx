"use client";

import {
  Notification03Icon,
  NotificationOff03Icon,
  TickDouble01Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { formatDistanceToNowStrict } from "date-fns";
import { useRouter } from "next/navigation";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import {
  useMarkAllNotificationsRead,
  useMarkNotificationRead,
  useNotificationStream,
  useNotifications,
  useUnreadNotificationCount,
} from "@/hooks/notifications";
import type { Notification } from "@/lib/types/notifications";
import { cn } from "@/lib/utils";

function timeAgo(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return formatDistanceToNowStrict(date, { addSuffix: true });
}

function NotificationRow({
  notification,
  onSelect,
}: {
  notification: Notification;
  onSelect: (notification: Notification) => void;
}) {
  return (
    <button
      type="button"
      onClick={() => onSelect(notification)}
      className={cn(
        "flex w-full gap-2.5 border-b border-border/60 px-3 py-2.5 text-left transition-colors last:border-b-0 hover:bg-muted/60",
        !notification.read && "bg-primary/[0.04]",
      )}
    >
      <span
        aria-hidden
        className={cn(
          "mt-1.5 size-1.5 shrink-0 rounded-full",
          notification.read ? "bg-transparent" : "bg-primary",
        )}
      />
      <span className="min-w-0 flex-1">
        <span className="flex items-baseline justify-between gap-2">
          <span
            className={cn(
              "truncate text-xs",
              notification.read ? "font-medium" : "font-semibold",
            )}
          >
            {notification.title}
          </span>
          <span className="shrink-0 text-[0.65rem] text-muted-foreground">
            {timeAgo(notification.createdAt)}
          </span>
        </span>
        {notification.body ? (
          <span className="mt-0.5 block text-[0.7rem] leading-relaxed text-muted-foreground">
            {notification.body}
          </span>
        ) : null}
      </span>
    </button>
  );
}

export function NotificationsButton() {
  const router = useRouter();
  const [open, setOpen] = useState(false);

  useNotificationStream();

  const { data: unreadCount = 0 } = useUnreadNotificationCount();
  const { data: notifications, isLoading } = useNotifications();
  const markRead = useMarkNotificationRead();
  const markAllRead = useMarkAllNotificationsRead();

  const items = notifications ?? [];

  function handleSelect(notification: Notification) {
    if (!notification.read) {
      markRead.mutate(notification.id);
    }
    if (notification.deepLink) {
      setOpen(false);
      router.push(notification.deepLink);
    }
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          className="relative"
          aria-label={
            unreadCount > 0
              ? `Notifications, ${unreadCount} unread`
              : "Notifications"
          }
        >
          <HugeiconsIcon
            icon={Notification03Icon}
            strokeWidth={2}
            className="size-4.5"
          />
          {unreadCount > 0 ? (
            <span className="absolute -top-0.5 -right-0.5 flex min-w-4 items-center justify-center rounded-full bg-primary px-1 text-[0.6rem] font-semibold text-primary-foreground tabular-nums">
              {unreadCount > 99 ? "99+" : unreadCount}
            </span>
          ) : null}
        </Button>
      </PopoverTrigger>

      <PopoverContent align="end" className="w-80 p-0">
        <div className="flex items-center justify-between border-b border-border/60 px-3 py-2">
          <p className="text-xs font-semibold">Notifications</p>
          {unreadCount > 0 ? (
            <Button
              variant="ghost"
              size="sm"
              className="h-6 gap-1 px-1.5 text-[0.7rem]"
              onClick={() => markAllRead.mutate()}
              disabled={markAllRead.isPending}
            >
              <HugeiconsIcon
                icon={TickDouble01Icon}
                strokeWidth={2}
                className="size-3"
              />
              Mark all read
            </Button>
          ) : null}
        </div>

        {isLoading ? (
          <div className="flex flex-col gap-2 p-3">
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-9 w-full" />
            <Skeleton className="h-9 w-full" />
          </div>
        ) : items.length === 0 ? (
          <div className="flex flex-col items-center gap-2 px-3 py-8 text-center">
            <span className="rounded-full bg-muted p-2.5">
              <HugeiconsIcon
                icon={NotificationOff03Icon}
                strokeWidth={2}
                className="size-4 text-muted-foreground"
              />
            </span>
            <p className="text-xs font-medium">You're all caught up</p>
            <p className="text-[0.7rem] text-muted-foreground">
              Fills, broken connections, and finished reports show up here.
            </p>
          </div>
        ) : (
          <ScrollArea className="[&>[data-radix-scroll-area-viewport]]:max-h-80 [&>[data-radix-scroll-area-viewport]>div]:!block">
            {items.map((notification) => (
              <NotificationRow
                key={notification.id}
                notification={notification}
                onSelect={handleSelect}
              />
            ))}
          </ScrollArea>
        )}
      </PopoverContent>
    </Popover>
  );
}
