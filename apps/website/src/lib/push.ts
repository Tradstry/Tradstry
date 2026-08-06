const SERVICE_WORKER_URL = "/sw.js";

export type PushKeys = {
  endpoint: string;
  p256dh: string;
  auth: string;
};

function urlBase64ToUint8Array(base64: string): Uint8Array<ArrayBuffer> {
  const padded = base64 + "=".repeat((4 - (base64.length % 4)) % 4);
  const normalized = padded.replace(/-/g, "+").replace(/_/g, "/");
  const raw = atob(normalized);
  const output = new Uint8Array(new ArrayBuffer(raw.length));
  for (let i = 0; i < raw.length; i += 1) {
    output[i] = raw.charCodeAt(i);
  }
  return output;
}

export function isPushSupported(): boolean {
  return (
    typeof window !== "undefined" &&
    "serviceWorker" in navigator &&
    "PushManager" in window &&
    "Notification" in window
  );
}

export function currentPermission(): NotificationPermission | "unsupported" {
  if (!isPushSupported()) return "unsupported";
  return Notification.permission;
}

export async function ensureServiceWorker(): Promise<ServiceWorkerRegistration> {
  const existing =
    await navigator.serviceWorker.getRegistration(SERVICE_WORKER_URL);
  if (existing) return existing;
  return navigator.serviceWorker.register(SERVICE_WORKER_URL);
}

export async function getBrowserSubscription(): Promise<PushSubscription | null> {
  if (!isPushSupported()) return null;
  const registration =
    await navigator.serviceWorker.getRegistration(SERVICE_WORKER_URL);
  if (!registration) return null;
  return registration.pushManager.getSubscription();
}

function readKeys(subscription: PushSubscription): PushKeys {
  const json = subscription.toJSON();
  const p256dh = json.keys?.p256dh;
  const auth = json.keys?.auth;
  if (!p256dh || !auth) {
    throw new Error("Push subscription is missing its encryption keys");
  }
  return { endpoint: subscription.endpoint, p256dh, auth };
}

export async function subscribeBrowser(publicKey: string): Promise<PushKeys> {
  if (!isPushSupported()) {
    throw new Error("This browser does not support push notifications");
  }

  const permission = await Notification.requestPermission();
  if (permission !== "granted") {
    throw new Error(
      permission === "denied"
        ? "Notifications are blocked for this site. Enable them in your browser settings."
        : "Notification permission was dismissed",
    );
  }

  const registration = await ensureServiceWorker();
  await navigator.serviceWorker.ready;

  const existing = await registration.pushManager.getSubscription();
  if (existing) return readKeys(existing);

  const subscription = await registration.pushManager.subscribe({
    userVisibleOnly: true,
    applicationServerKey: urlBase64ToUint8Array(publicKey),
  });
  return readKeys(subscription);
}

/** Returns the endpoint that was torn down so the caller can drop it server-side. */
export async function unsubscribeBrowser(): Promise<string | null> {
  const subscription = await getBrowserSubscription();
  if (!subscription) return null;
  const { endpoint } = subscription;
  await subscription.unsubscribe();
  return endpoint;
}
