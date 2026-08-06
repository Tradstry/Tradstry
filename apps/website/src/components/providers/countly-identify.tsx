"use client";

import { useUser } from "@clerk/nextjs";
import { useEffect, useRef } from "react";
import { Countly, countlyEnabled } from "@/lib/analytics/countly";

export function CountlyIdentify() {
  const { isLoaded, isSignedIn, user } = useUser();
  // Avoid churning the anonymous Countly device ID on every landing-page mount.
  const wasSignedIn = useRef(false);

  useEffect(() => {
    if (!isLoaded || !countlyEnabled()) {
      return;
    }
    if (isSignedIn && user) {
      // Merge pre-sign-in activity into the Clerk identity so backend events
      // (which use the Clerk ID too) land on this same Countly profile.
      Countly.change_id(user.id, true);
      Countly.user_details({
        email: user.primaryEmailAddress?.emailAddress,
        name: user.fullName ?? undefined,
        custom: { created_at: user.createdAt?.toISOString() },
      });
      wasSignedIn.current = true;
      return;
    }
    if (wasSignedIn.current) {
      Countly.set_id(`anonymous-${crypto.randomUUID()}`);
      wasSignedIn.current = false;
    }
  }, [isLoaded, isSignedIn, user]);

  return null;
}
