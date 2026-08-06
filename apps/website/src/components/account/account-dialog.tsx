"use client";

import { UserCircle02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { DangerSection } from "@/components/account/danger-section";
import { EmailSection } from "@/components/account/email-section";
import { ExportSection } from "@/components/account/export-section";
import { NotificationsSection } from "@/components/account/notifications-section";
import { ProfileSection } from "@/components/account/profile-section";
import { SecuritySection } from "@/components/account/security-section";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@tradstry/app-ui/components/ui/dialog";
import { ScrollArea } from "@tradstry/app-ui/components/ui/scroll-area";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@tradstry/app-ui/components/ui/tabs";

const TABS = [
  { value: "profile", label: "Profile", render: () => <ProfileSection /> },
  { value: "email", label: "Email", render: () => <EmailSection /> },
  {
    value: "notifications",
    label: "Notifications",
    render: () => <NotificationsSection />,
  },
  { value: "security", label: "Security", render: () => <SecuritySection /> },
  {
    value: "danger",
    label: "Danger",
    render: () => (
      <div className="grid gap-4">
        <ExportSection />
        <DangerSection />
      </div>
    ),
  },
] as const;

export function AccountDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [tab, setTab] = React.useState<string>("profile");

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="flex h-[min(40rem,calc(100svh-2rem))] flex-col overflow-hidden sm:max-w-2xl">
        <DialogHeader className="shrink-0">
          <DialogTitle className="flex items-center gap-2">
            <span className="flex size-7 items-center justify-center rounded-md bg-muted text-muted-foreground">
              <HugeiconsIcon
                icon={UserCircle02Icon}
                strokeWidth={2}
                className="size-4"
              />
            </span>
            Account
          </DialogTitle>
          <DialogDescription>
            Manage your profile, sign-in methods and devices.
          </DialogDescription>
        </DialogHeader>

        <Tabs
          value={tab}
          onValueChange={setTab}
          className="flex min-h-0 flex-1 flex-col overflow-hidden"
        >
          <TabsList className="shrink-0">
            {TABS.map((item) => (
              <TabsTrigger
                key={item.value}
                value={item.value}
                className={
                  item.value === "danger"
                    ? "data-[state=active]:text-destructive"
                    : undefined
                }
              >
                {item.label}
              </TabsTrigger>
            ))}
          </TabsList>

          {TABS.map((item) => (
            <TabsContent
              key={item.value}
              value={item.value}
              className="min-h-0 flex-1 overflow-hidden"
            >
              <ScrollArea className="h-full">
                <div className="pr-3 pb-1">{item.render()}</div>
              </ScrollArea>
            </TabsContent>
          ))}
        </Tabs>
      </DialogContent>
    </Dialog>
  );
}
