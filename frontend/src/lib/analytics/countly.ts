"use client";

import Countly from "countly-sdk-web";

let initialized = false;

export function countlyEnabled() {
  return Boolean(
    process.env.NEXT_PUBLIC_COUNTLY_APP_KEY &&
      process.env.NEXT_PUBLIC_COUNTLY_HOST,
  );
}

export function initializeCountly() {
  if (initialized || !countlyEnabled()) {
    return false;
  }

  Countly.init({
    app_key: process.env.NEXT_PUBLIC_COUNTLY_APP_KEY,
    // Require an explicit host so production events cannot accidentally be sent
    // to Countly Cloud when this app is intended to use Countly Lite on-premise.
    url: process.env.NEXT_PUBLIC_COUNTLY_HOST,
  });
  Countly.track_sessions();
  Countly.track_errors({});
  initialized = true;
  return true;
}

export { Countly };
