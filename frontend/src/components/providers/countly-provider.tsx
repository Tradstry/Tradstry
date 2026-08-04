"use client";

import { usePathname } from "next/navigation";
import { useEffect } from "react";
import {
  Countly,
  countlyEnabled,
  initializeCountly,
} from "@/lib/analytics/countly";

export function CountlyProvider({ children }: { children: React.ReactNode }) {
  const pathname = usePathname();

  useEffect(() => {
    initializeCountly();
  }, []);

  useEffect(() => {
    if (pathname && countlyEnabled()) {
      Countly.track_pageview(pathname);
    }
  }, [pathname]);

  return <>{children}</>;
}
