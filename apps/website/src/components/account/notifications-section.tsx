"use client";

import { toast } from "sonner";
import { Field, Section, Spinner } from "@/components/account/shared";
import { Button } from "@tradstry/app-ui/components/ui/button";
import { Input } from "@tradstry/app-ui/components/ui/input";
import { Label } from "@tradstry/app-ui/components/ui/label";
import { Switch } from "@tradstry/app-ui/components/ui/switch";
import {
  useNotificationPreferences,
  useNotificationSettings,
  useSetNotificationPreference,
  useSetNotificationSettings,
  useTimezoneSync,
  useWebPush,
} from "@tradstry/app-ui/hooks/notifications";
import {
  DAYS_OF_WEEK,
  minuteToTime,
  NOTIFICATION_EVENT_LABELS,
  type NotificationEventType,
  type NotificationSettingsPatch,
  timeToMinute,
} from "@tradstry/app-ui/lib/types/notifications";

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

function ScheduleSection() {
  const { data: settings, isLoading } = useNotificationSettings();
  const setSettings = useSetNotificationSettings();

  function patch(next: NotificationSettingsPatch) {
    setSettings.mutate(next, {
      onError: (err) =>
        toast.error(errorMessage(err, "Could not update schedule")),
    });
  }

  function onTimeChange(field: "dailyRecapMinute" | "weeklyReviewMinute") {
    return (event: React.ChangeEvent<HTMLInputElement>) => {
      const minute = timeToMinute(event.target.value);
      if (minute !== null) patch({ [field]: minute });
    };
  }

  const quietOn =
    settings?.quietStartMinute != null && settings?.quietEndMinute != null;

  return (
    <Section
      title="When"
      description="Scheduled notifications use your local time. Quiet hours hold pushes back until the window ends — nothing is lost, it just waits."
    >
      {isLoading || !settings ? (
        <Spinner />
      ) : (
        <div className="grid gap-4">
          <Field label="Time zone" htmlFor="tz">
            <p id="tz" className="text-xs">
              {settings.timezone}
            </p>
          </Field>

          <Field label="Daily reminder, after the close" htmlFor="recap-at">
            <Input
              id="recap-at"
              type="time"
              className="w-32"
              defaultValue={minuteToTime(settings.dailyRecapMinute)}
              onChange={onTimeChange("dailyRecapMinute")}
            />
          </Field>

          <div className="flex items-end gap-2">
            <Field label="Weekly review" htmlFor="review-dow">
              <select
                id="review-dow"
                className="h-9 rounded-md border border-input bg-transparent px-2 text-xs"
                value={settings.weeklyReviewDow}
                onChange={(e) =>
                  patch({ weeklyReviewDow: Number(e.target.value) })
                }
              >
                {DAYS_OF_WEEK.map((day, i) => (
                  <option key={day} value={i}>
                    {day}
                  </option>
                ))}
              </select>
            </Field>
            <Input
              aria-label="Weekly review time"
              type="time"
              className="w-32"
              defaultValue={minuteToTime(settings.weeklyReviewMinute)}
              onChange={onTimeChange("weeklyReviewMinute")}
            />
          </div>

          <div className="flex items-start justify-between gap-4 border-t border-border/60 pt-4">
            <div className="min-w-0">
              <Label htmlFor="quiet" className="text-xs font-medium">
                Quiet hours
              </Label>
              <p className="mt-0.5 text-xs leading-relaxed text-muted-foreground">
                Hold pushes overnight. They arrive when the window ends.
              </p>
            </div>
            <Switch
              id="quiet"
              checked={quietOn}
              onCheckedChange={(on) =>
                patch(
                  on
                    ? { quietStartMinute: 1320, quietEndMinute: 420 }
                    : { quietStartMinute: null, quietEndMinute: null },
                )
              }
            />
          </div>

          {quietOn ? (
            <div className="flex items-end gap-2">
              <Field label="From" htmlFor="quiet-from">
                <Input
                  id="quiet-from"
                  type="time"
                  className="w-32"
                  defaultValue={minuteToTime(settings.quietStartMinute ?? 1320)}
                  onChange={(e) => {
                    const m = timeToMinute(e.target.value);
                    if (m !== null) patch({ quietStartMinute: m });
                  }}
                />
              </Field>
              <Field label="To" htmlFor="quiet-to">
                <Input
                  id="quiet-to"
                  type="time"
                  className="w-32"
                  defaultValue={minuteToTime(settings.quietEndMinute ?? 420)}
                  onChange={(e) => {
                    const m = timeToMinute(e.target.value);
                    if (m !== null) patch({ quietEndMinute: m });
                  }}
                />
              </Field>
            </div>
          ) : null}
        </div>
      )}
    </Section>
  );
}

export function NotificationsSection() {
  useTimezoneSync();

  return (
    <div className="grid gap-4">
      <PushSection />
      <ScheduleSection />
      <PreferencesSection />
    </div>
  );
}
