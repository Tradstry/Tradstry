export const NOTIFICATION_EVENT_TYPES = [
  "FillsLanded",
  "BrokerageConnectionDisabled",
  "ArtifactReady",
  "PrincipleViolated",
  "DailyRecap",
  "WeeklyReview",
] as const;

export type NotificationEventType = (typeof NOTIFICATION_EVENT_TYPES)[number];

export type Notification = {
  id: string;
  eventType: string;
  title: string;
  body: string;
  deepLink: string | null;
  groupCount: number;
  read: boolean;
  createdAt: string;
  updatedAt: string;
};

export type NotificationPreference = {
  eventType: string;
  enabled: boolean;
};

export type PushSubscriptionSummary = {
  endpoint: string;
};

export type RegisterPushSubscriptionInput = {
  endpoint: string;
  p256dh: string;
  auth: string;
  userAgent?: string;
};

export const NOTIFICATION_EVENT_LABELS: Record<
  NotificationEventType,
  { label: string; description: string }
> = {
  FillsLanded: {
    label: "New fills",
    description:
      "When a brokerage sync brings in trades you haven't journaled.",
  },
  BrokerageConnectionDisabled: {
    label: "Broken connections",
    description: "When a brokerage stops syncing and needs reconnecting.",
  },
  ArtifactReady: {
    label: "Reports and insights",
    description: "When an AI report, insight, or mindset summary finishes.",
  },
  PrincipleViolated: {
    label: "Principle violations",
    description: "When a trade breaks one of your trading principles.",
  },
  DailyRecap: {
    label: "Daily journaling reminder",
    description:
      "After the close on weekdays, when you have trades that aren't written up.",
  },
  WeeklyReview: {
    label: "Weekly review",
    description:
      "A summary of how you traded — journaling, rule adherence, and holding patterns.",
  },
};

export type NotificationSettings = {
  timezone: string;
  dailyRecapMinute: number;
  weeklyReviewDow: number;
  weeklyReviewMinute: number;
  quietStartMinute: number | null;
  quietEndMinute: number | null;
};

export type NotificationSettingsPatch = Partial<NotificationSettings>;

export const DAYS_OF_WEEK = [
  "Sunday",
  "Monday",
  "Tuesday",
  "Wednesday",
  "Thursday",
  "Friday",
  "Saturday",
] as const;

export function minuteToTime(minute: number): string {
  const h = Math.floor(minute / 60);
  const m = minute % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

export function timeToMinute(value: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(value);
  if (!match) return null;
  const minute = Number(match[1]) * 60 + Number(match[2]);
  return minute >= 0 && minute <= 1439 ? minute : null;
}

export function notificationEventLabel(eventType: string): string {
  return (
    NOTIFICATION_EVENT_LABELS[eventType as NotificationEventType]?.label ??
    eventType
  );
}
