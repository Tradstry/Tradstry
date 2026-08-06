import type { GraphQLFetcher } from "@tradstry/app-ui/lib/client";
import type {
  Notification,
  NotificationPreference,
  NotificationSettings,
  NotificationSettingsPatch,
  PushSubscriptionSummary,
  RegisterPushSubscriptionInput,
} from "@tradstry/app-ui/lib/types/notifications";

const NOTIFICATION_FIELDS = `
  id
  eventType
  title
  body
  deepLink
  groupCount
  read
  createdAt
  updatedAt
`;

const NOTIFICATIONS_QUERY = `
  query Notifications($limit: Int, $before: String) {
    notifications(limit: $limit, before: $before) {
      ${NOTIFICATION_FIELDS}
    }
  }
`;

const UNREAD_COUNT_QUERY = `
  query UnreadNotificationCount {
    unreadNotificationCount
  }
`;

const PREFERENCES_QUERY = `
  query NotificationPreferences {
    notificationPreferences {
      eventType
      enabled
    }
  }
`;

const WEB_PUSH_PUBLIC_KEY_QUERY = `
  query WebPushPublicKey {
    webPushPublicKey
  }
`;

const PUSH_SUBSCRIPTIONS_QUERY = `
  query PushSubscriptions {
    pushSubscriptions {
      endpoint
    }
  }
`;

const SETTINGS_FIELDS = `
  timezone
  dailyRecapMinute
  weeklyReviewDow
  weeklyReviewMinute
  quietStartMinute
  quietEndMinute
`;

const SETTINGS_QUERY = `
  query NotificationSettings {
    notificationSettings {
      ${SETTINGS_FIELDS}
    }
  }
`;

const SET_SETTINGS_MUTATION = `
  mutation SetNotificationSettings(
    $timezone: String
    $dailyRecapMinute: Int
    $weeklyReviewDow: Int
    $weeklyReviewMinute: Int
    $quietStartMinute: Int
    $quietEndMinute: Int
  ) {
    setNotificationSettings(
      timezone: $timezone
      dailyRecapMinute: $dailyRecapMinute
      weeklyReviewDow: $weeklyReviewDow
      weeklyReviewMinute: $weeklyReviewMinute
      quietStartMinute: $quietStartMinute
      quietEndMinute: $quietEndMinute
    ) {
      ${SETTINGS_FIELDS}
    }
  }
`;

const MARK_READ_MUTATION = `
  mutation MarkNotificationRead($id: String!) {
    markNotificationRead(id: $id)
  }
`;

const MARK_ALL_READ_MUTATION = `
  mutation MarkAllNotificationsRead {
    markAllNotificationsRead
  }
`;

const SET_PREFERENCE_MUTATION = `
  mutation SetNotificationPreference($eventType: String!, $enabled: Boolean!) {
    setNotificationPreference(eventType: $eventType, enabled: $enabled)
  }
`;

// `p256Dh`, not `p256dh`: async-graphql treats the digits as a word boundary
// when it camelCases the Rust argument name.
const REGISTER_PUSH_MUTATION = `
  mutation RegisterPushSubscription(
    $endpoint: String!
    $p256Dh: String!
    $auth: String!
    $userAgent: String
  ) {
    registerPushSubscription(
      endpoint: $endpoint
      p256Dh: $p256Dh
      auth: $auth
      userAgent: $userAgent
    )
  }
`;

const DELETE_PUSH_MUTATION = `
  mutation DeletePushSubscription($endpoint: String!) {
    deletePushSubscription(endpoint: $endpoint)
  }
`;

export const NOTIFICATION_EVENTS_SUBSCRIPTION = `
  subscription NotificationEvents {
    notificationEvents
  }
`;

export async function fetchNotifications(
  fetcher: GraphQLFetcher,
  limit?: number,
  before?: string,
): Promise<Notification[]> {
  const data = await fetcher<{ notifications: Notification[] }>(
    NOTIFICATIONS_QUERY,
    { limit, before },
  );
  return data.notifications;
}

export async function fetchUnreadCount(
  fetcher: GraphQLFetcher,
): Promise<number> {
  const data = await fetcher<{ unreadNotificationCount: number }>(
    UNREAD_COUNT_QUERY,
  );
  return data.unreadNotificationCount;
}

export async function fetchPreferences(
  fetcher: GraphQLFetcher,
): Promise<NotificationPreference[]> {
  const data = await fetcher<{
    notificationPreferences: NotificationPreference[];
  }>(PREFERENCES_QUERY);
  return data.notificationPreferences;
}

export async function fetchWebPushPublicKey(
  fetcher: GraphQLFetcher,
): Promise<string | null> {
  const data = await fetcher<{ webPushPublicKey: string | null }>(
    WEB_PUSH_PUBLIC_KEY_QUERY,
  );
  return data.webPushPublicKey;
}

export async function fetchPushSubscriptions(
  fetcher: GraphQLFetcher,
): Promise<PushSubscriptionSummary[]> {
  const data = await fetcher<{ pushSubscriptions: PushSubscriptionSummary[] }>(
    PUSH_SUBSCRIPTIONS_QUERY,
  );
  return data.pushSubscriptions;
}

export async function fetchNotificationSettings(
  fetcher: GraphQLFetcher,
): Promise<NotificationSettings> {
  const data = await fetcher<{ notificationSettings: NotificationSettings }>(
    SETTINGS_QUERY,
  );
  return data.notificationSettings;
}

/// Omitted keys stay untouched server-side; an explicit `null` clears the field.
export async function setNotificationSettings(
  fetcher: GraphQLFetcher,
  patch: NotificationSettingsPatch,
): Promise<NotificationSettings> {
  const data = await fetcher<{ setNotificationSettings: NotificationSettings }>(
    SET_SETTINGS_MUTATION,
    patch as Record<string, unknown>,
  );
  return data.setNotificationSettings;
}

export async function markNotificationRead(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ markNotificationRead: boolean }>(
    MARK_READ_MUTATION,
    { id },
  );
  return data.markNotificationRead;
}

export async function markAllNotificationsRead(
  fetcher: GraphQLFetcher,
): Promise<number> {
  const data = await fetcher<{ markAllNotificationsRead: number }>(
    MARK_ALL_READ_MUTATION,
  );
  return data.markAllNotificationsRead;
}

export async function setNotificationPreference(
  fetcher: GraphQLFetcher,
  eventType: string,
  enabled: boolean,
): Promise<boolean> {
  const data = await fetcher<{ setNotificationPreference: boolean }>(
    SET_PREFERENCE_MUTATION,
    { eventType, enabled },
  );
  return data.setNotificationPreference;
}

export async function registerPushSubscription(
  fetcher: GraphQLFetcher,
  input: RegisterPushSubscriptionInput,
): Promise<boolean> {
  const data = await fetcher<{ registerPushSubscription: boolean }>(
    REGISTER_PUSH_MUTATION,
    {
      endpoint: input.endpoint,
      p256Dh: input.p256dh,
      auth: input.auth,
      userAgent: input.userAgent,
    },
  );
  return data.registerPushSubscription;
}

export async function deletePushSubscription(
  fetcher: GraphQLFetcher,
  endpoint: string,
): Promise<boolean> {
  const data = await fetcher<{ deletePushSubscription: boolean }>(
    DELETE_PUSH_MUTATION,
    { endpoint },
  );
  return data.deletePushSubscription;
}
