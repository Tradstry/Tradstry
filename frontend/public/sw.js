/* Tradstry web-push service worker. */

self.addEventListener("install", () => {
  self.skipWaiting();
});

self.addEventListener("activate", (event) => {
  event.waitUntil(self.clients.claim());
});

self.addEventListener("push", (event) => {
  if (!event.data) return;

  let payload;
  try {
    payload = event.data.json();
  } catch {
    payload = { title: "Tradstry", body: event.data.text() };
  }

  const deepLink = payload.deep_link || "/dashboard";

  event.waitUntil(
    self.registration.showNotification(payload.title || "Tradstry", {
      body: payload.body || "",
      icon: "/icon-192.png",
      badge: "/icon-192.png",
      // The backend coalesces events into one row, so tagging by id lets an
      // updated group replace its own toast instead of stacking a second one.
      tag: payload.notification_id || undefined,
      renotify: Boolean(payload.notification_id),
      data: { deepLink, notificationId: payload.notification_id },
    }),
  );
});

self.addEventListener("notificationclick", (event) => {
  event.notification.close();

  const deepLink = event.notification.data?.deepLink || "/dashboard";
  const target = new URL(deepLink, self.location.origin);

  event.waitUntil(
    self.clients
      .matchAll({ type: "window", includeUncontrolled: true })
      .then((clients) => {
        for (const client of clients) {
          if (new URL(client.url).origin === target.origin && "focus" in client) {
            client.navigate(target.href);
            return client.focus();
          }
        }
        return self.clients.openWindow(target.href);
      }),
  );
});
