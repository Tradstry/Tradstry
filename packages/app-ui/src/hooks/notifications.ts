"use client";

import {
	type QueryClient,
	useMutation,
	useQuery,
	useQueryClient,
} from "@tanstack/react-query";
import {
	useGraphQL,
	useGraphQLSubscription,
} from "@tradstry/app-ui/lib/client";
import * as push from "@tradstry/app-ui/lib/push";
import * as notificationService from "@tradstry/app-ui/lib/service/notifications";
import type {
	Notification,
	NotificationPreference,
	NotificationSettings,
	NotificationSettingsPatch,
	PushSubscriptionSummary,
} from "@tradstry/app-ui/lib/types/notifications";
import { useAuth, useTradstryPlatform } from "@tradstry/app-ui/platform";
import { useEffect, useRef, useState } from "react";
import { type OptimisticContext, optimisticUpdate } from "./optimistic";

// `unread-count` sits under the feed prefix on purpose: one invalidation of
// ["notifications"] reconciles the list and the badge together.
const FEED_KEY = ["notifications"] as const;
const UNREAD_KEY = ["notifications", "unread-count"] as const;
const PREFERENCES_KEY = ["notification-preferences"] as const;
const PUSH_SERVER_KEY = ["push-subscriptions"] as const;
const PUSH_BROWSER_KEY = ["push-subscriptions", "browser"] as const;
const PUSH_PUBLIC_KEY = ["web-push-public-key"] as const;
const SETTINGS_KEY = ["notification-settings"] as const;

function findCached(qc: QueryClient, id: string): Notification | undefined {
	for (const [, data] of qc.getQueriesData({ queryKey: FEED_KEY })) {
		if (!Array.isArray(data)) continue;
		const hit = (data as Notification[]).find((n) => n.id === id);
		if (hit) return hit;
	}
	return undefined;
}

function setUnread(qc: QueryClient, fn: (count: number) => number): void {
	qc.setQueryData<number>(UNREAD_KEY, (old) =>
		typeof old === "number" ? fn(old) : old,
	);
}

export function useNotifications(limit = 30) {
	const { isLoaded, isSignedIn } = useAuth();
	const fetcher = useGraphQL();

	return useQuery<Notification[]>({
		queryKey: [...FEED_KEY, "feed", limit],
		queryFn: () => notificationService.fetchNotifications(fetcher, limit),
		enabled: isLoaded && isSignedIn,
	});
}

export function useUnreadNotificationCount() {
	const { isLoaded, isSignedIn } = useAuth();
	const fetcher = useGraphQL();

	return useQuery<number>({
		queryKey: UNREAD_KEY,
		queryFn: () => notificationService.fetchUnreadCount(fetcher),
		enabled: isLoaded && isSignedIn,
	});
}

export function useMarkNotificationRead() {
	const fetcher = useGraphQL();
	const queryClient = useQueryClient();

	const handlers = optimisticUpdate<string, Notification>(
		queryClient,
		FEED_KEY,
		(id) => id,
		(entity) => ({ ...entity, read: true }),
	);

	return useMutation({
		mutationFn: (id: string) =>
			notificationService.markNotificationRead(fetcher, id),
		...handlers,
		onMutate: async (id: string) => {
			const wasUnread = findCached(queryClient, id)?.read === false;
			const ctx = await handlers.onMutate(id);
			if (wasUnread) setUnread(queryClient, (n) => Math.max(0, n - 1));
			return ctx;
		},
	});
}

export function useMarkAllNotificationsRead() {
	const fetcher = useGraphQL();
	const queryClient = useQueryClient();

	return useMutation({
		mutationFn: () => notificationService.markAllNotificationsRead(fetcher),
		onMutate: async (): Promise<OptimisticContext> => {
			await queryClient.cancelQueries({ queryKey: FEED_KEY });
			const snapshots = queryClient
				.getQueriesData({ queryKey: FEED_KEY })
				.map(([key, data]) => [key, data] as [readonly unknown[], unknown]);

			queryClient.setQueriesData<unknown>(
				{ queryKey: FEED_KEY },
				(old: unknown) =>
					Array.isArray(old)
						? (old as Notification[]).map((n) => ({ ...n, read: true }))
						: old,
			);
			setUnread(queryClient, () => 0);

			return { snapshots };
		},
		onError: (_error, _vars, ctx: OptimisticContext | undefined) => {
			for (const [key, data] of ctx?.snapshots ?? []) {
				queryClient.setQueryData(key, data);
			}
		},
		onSettled: () => {
			queryClient.invalidateQueries({ queryKey: FEED_KEY });
		},
	});
}

/**
 * Live badge updates. The feed query stays the source of truth — a dropped
 * broadcast just means the badge waits for the next refetch.
 */
export function useNotificationStream() {
	const { isLoaded, isSignedIn } = useAuth();
	const subscriber = useGraphQLSubscription();
	const queryClient = useQueryClient();
	const fetcher = useGraphQL();
	const platform = useTradstryPlatform();
	const displayed = useRef(new Set<string>());

	useEffect(() => {
		if (!isLoaded || !isSignedIn) return;

		return subscriber<{ notificationEvents: string }>(
			notificationService.NOTIFICATION_EVENTS_SUBSCRIPTION,
			undefined,
			{
				onMessage: (payload) => {
					queryClient.invalidateQueries({ queryKey: FEED_KEY });
					const id = payload.notificationEvents;
					if (displayed.current.has(id)) return;
					displayed.current.add(id);
					void notificationService
						.fetchNotifications(fetcher, 10)
						.then((items) => {
							const item = items.find((candidate) => candidate.id === id);
							if (
								!item ||
								typeof window === "undefined" ||
								!("Notification" in window)
							)
								return;
							if (window.Notification.permission !== "granted") return;
							const notification = new window.Notification(item.title, {
								body: item.body,
							});
							notification.onclick = () => {
								window.focus();
								if (item.deepLink) platform.navigate(item.deepLink);
								notification.close();
							};
						});
				},
			},
		);
	}, [isLoaded, isSignedIn, subscriber, queryClient, fetcher, platform]);
}

export function useNotificationPreferences() {
	const { isLoaded, isSignedIn } = useAuth();
	const fetcher = useGraphQL();

	return useQuery<NotificationPreference[]>({
		queryKey: PREFERENCES_KEY,
		queryFn: () => notificationService.fetchPreferences(fetcher),
		enabled: isLoaded && isSignedIn,
	});
}

export function useSetNotificationPreference() {
	const fetcher = useGraphQL();
	const queryClient = useQueryClient();

	type Vars = { eventType: string; enabled: boolean };

	return useMutation({
		mutationFn: ({ eventType, enabled }: Vars) =>
			notificationService.setNotificationPreference(
				fetcher,
				eventType,
				enabled,
			),
		onMutate: async ({ eventType, enabled }: Vars) => {
			await queryClient.cancelQueries({ queryKey: PREFERENCES_KEY });
			const previous =
				queryClient.getQueryData<NotificationPreference[]>(PREFERENCES_KEY);

			queryClient.setQueryData<NotificationPreference[]>(
				PREFERENCES_KEY,
				(old) =>
					old?.map((p) => (p.eventType === eventType ? { ...p, enabled } : p)),
			);

			return { previous };
		},
		onError: (_error, _vars, ctx) => {
			if (ctx?.previous) {
				queryClient.setQueryData(PREFERENCES_KEY, ctx.previous);
			}
		},
		onSettled: () => {
			queryClient.invalidateQueries({ queryKey: PREFERENCES_KEY });
		},
	});
}

export function useNotificationSettings() {
	const { isLoaded, isSignedIn } = useAuth();
	const fetcher = useGraphQL();

	return useQuery<NotificationSettings>({
		queryKey: SETTINGS_KEY,
		queryFn: () => notificationService.fetchNotificationSettings(fetcher),
		enabled: isLoaded && isSignedIn,
	});
}

export function useSetNotificationSettings() {
	const fetcher = useGraphQL();
	const queryClient = useQueryClient();

	return useMutation({
		mutationFn: (patch: NotificationSettingsPatch) =>
			notificationService.setNotificationSettings(fetcher, patch),
		onSuccess: (next) => {
			queryClient.setQueryData(SETTINGS_KEY, next);
		},
		onSettled: () => {
			queryClient.invalidateQueries({ queryKey: SETTINGS_KEY });
		},
	});
}

/**
 * Writes the browser's timezone once when it disagrees with what the server has.
 * Without it every schedule would run on the ET default, so a trader in London
 * would get their "after the close" nudge at 21:15 local.
 */
export function useTimezoneSync() {
	const { data: settings } = useNotificationSettings();
	const setSettings = useSetNotificationSettings();
	const attempted = useRef(false);

	useEffect(() => {
		if (!settings || attempted.current) return;

		const browserTz = Intl.DateTimeFormat().resolvedOptions().timeZone;
		if (!browserTz || browserTz === settings.timezone) return;

		attempted.current = true;
		setSettings.mutate({ timezone: browserTz });
	}, [settings, setSettings]);
}

/**
 * Web push for this device. "Enabled" means the browser holds a subscription
 * *and* the server still knows about it — a subscription the backend pruned
 * after repeated 410s would otherwise look active but deliver nothing.
 */
export function useWebPush() {
	const { isLoaded, isSignedIn } = useAuth();
	const fetcher = useGraphQL();
	const queryClient = useQueryClient();

	const [supported, setSupported] = useState(false);
	const [permission, setPermission] = useState<
		NotificationPermission | "unsupported"
	>("unsupported");

	useEffect(() => {
		setSupported(push.isPushSupported());
		setPermission(push.currentPermission());
	}, []);

	const publicKey = useQuery<string | null>({
		queryKey: PUSH_PUBLIC_KEY,
		queryFn: () => notificationService.fetchWebPushPublicKey(fetcher),
		enabled: isLoaded && isSignedIn,
		staleTime: Number.POSITIVE_INFINITY,
	});

	const browserEndpoint = useQuery<string | null>({
		queryKey: PUSH_BROWSER_KEY,
		queryFn: async () =>
			(await push.getBrowserSubscription())?.endpoint ?? null,
		enabled: supported,
	});

	const serverSubscriptions = useQuery<PushSubscriptionSummary[]>({
		queryKey: PUSH_SERVER_KEY,
		queryFn: () => notificationService.fetchPushSubscriptions(fetcher),
		enabled: isLoaded && isSignedIn && supported,
	});

	const refresh = () => {
		queryClient.invalidateQueries({ queryKey: PUSH_SERVER_KEY });
		setPermission(push.currentPermission());
	};

	const enable = useMutation({
		mutationFn: async () => {
			if (!publicKey.data) {
				throw new Error("Push notifications aren't configured on the server");
			}
			const keys = await push.subscribeBrowser(publicKey.data);
			await notificationService.registerPushSubscription(fetcher, {
				...keys,
				userAgent: navigator.userAgent,
			});
			return keys.endpoint;
		},
		onSuccess: refresh,
		onError: () => setPermission(push.currentPermission()),
	});

	const disable = useMutation({
		mutationFn: async () => {
			const endpoint = await push.unsubscribeBrowser();
			if (endpoint) {
				await notificationService.deletePushSubscription(fetcher, endpoint);
			}
		},
		onSuccess: refresh,
	});

	const endpoint = browserEndpoint.data ?? null;
	const enabled =
		!!endpoint &&
		(serverSubscriptions.data?.some((s) => s.endpoint === endpoint) ?? false);

	return {
		supported,
		permission,
		configured: !!publicKey.data,
		enabled,
		isLoading:
			browserEndpoint.isLoading ||
			serverSubscriptions.isLoading ||
			publicKey.isLoading,
		isPending: enable.isPending || disable.isPending,
		error: enable.error ?? disable.error ?? null,
		enable: enable.mutateAsync,
		disable: disable.mutateAsync,
	};
}
