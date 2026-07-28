"use client";

import { toast } from "sonner";
import { Section, Spinner } from "@/components/account/shared";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  useNotificationPreferences,
  useSetNotificationPreference,
  useWebPush,
} from "@/hooks/notifications";
import {
  NOTIFICATION_EVENT_LABELS,
  type NotificationEventType,
} from "@/lib/types/notifications";

function errorMessage(error: unknown, fallback: string): string {
  return error instanceof Error ? error.message : fallback;
}

function PushSection() {
  const push = useWebPush();

  async function toggle() {
    try {
      if (push.enabled) {
        await push.disable();
        toast.success("Push notifications turned off for this device");
      } else {
        await push.enable();
        toast.success("Push notifications turned on for this device");
      }
    } catch (err) {
      toast.error(errorMessage(err, "Could not update push notifications"));
    }
  }

  const blocked = push.permission === "denied";

  return (
    <Section
      title="This device"
      description="Push notifications reach you when Tradstry isn't open. They apply to this browser only — turn them on again on each device you use."
    >
      {!push.supported ? (
        <p className="text-xs text-muted-foreground">
          This browser doesn't support push notifications.
        </p>
      ) : !push.configured ? (
        <p className="text-xs text-muted-foreground">
          Push isn't configured on the server yet.
        </p>
      ) : push.isLoading ? (
        <Spinner />
      ) : (
        <div className="flex items-center justify-between gap-4">
          <div className="min-w-0">
            <p className="text-xs font-medium">
              {push.enabled ? "Enabled on this device" : "Not enabled here"}
            </p>
            {blocked ? (
              <p className="mt-1 text-xs text-destructive">
                Notifications are blocked for this site. Allow them in your
                browser settings, then try again.
              </p>
            ) : null}
          </div>
          <Button
            variant={push.enabled ? "outline" : "default"}
            size="sm"
            onClick={toggle}
            disabled={push.isPending || (blocked && !push.enabled)}
          >
            {push.isPending ? "..." : push.enabled ? "Turn off" : "Turn on"}
          </Button>
        </div>
      )}
    </Section>
  );
}

function PreferencesSection() {
  const { data: preferences, isLoading } = useNotificationPreferences();
  const setPreference = useSetNotificationPreference();

  return (
    <Section
      title="What to notify me about"
      description="Applies everywhere — in-app and push. Turning one off stops new notifications of that kind from being created."
    >
      {isLoading ? (
        <Spinner />
      ) : (
        <div className="grid gap-4">
          {(preferences ?? []).map((preference) => {
            const meta =
              NOTIFICATION_EVENT_LABELS[
                preference.eventType as NotificationEventType
              ];
            const id = `notification-pref-${preference.eventType}`;

            return (
              <div
                key={preference.eventType}
                className="flex items-start justify-between gap-4"
              >
                <div className="min-w-0">
                  <Label htmlFor={id} className="text-xs font-medium">
                    {meta?.label ?? preference.eventType}
                  </Label>
                  {meta ? (
                    <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                      {meta.description}
                    </p>
                  ) : null}
                </div>
                <Switch
                  id={id}
                  checked={preference.enabled}
                  onCheckedChange={(enabled) =>
                    setPreference.mutate({
                      eventType: preference.eventType,
                      enabled,
                    })
                  }
                />
              </div>
            );
          })}
        </div>
      )}
    </Section>
  );
}

export function NotificationsSection() {
  return (
    <div className="grid gap-4">
      <PushSection />
      <PreferencesSection />
    </div>
  );
}
