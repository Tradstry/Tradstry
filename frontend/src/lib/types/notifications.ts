export const NOTIFICATION_EVENT_TYPES = [
  "FillsLanded",
  "BrokerageConnectionDisabled",
  "ArtifactReady",
  "PrincipleViolated",
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
};

export function notificationEventLabel(eventType: string): string {
  return (
    NOTIFICATION_EVENT_LABELS[eventType as NotificationEventType]?.label ??
    eventType
  );
}
