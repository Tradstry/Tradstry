import type { MetadataRoute } from "next";
import { CONTENT_LAST_MODIFIED, PUBLIC_ROUTES, SITE_URL } from "@/lib/site";

export default function sitemap(): MetadataRoute.Sitemap {
  return PUBLIC_ROUTES.map((route) => ({
    url: `${SITE_URL}${route === "/" ? "" : route}`,
    lastModified: CONTENT_LAST_MODIFIED,
    changeFrequency: route === "/" ? "weekly" : "yearly",
    priority: route === "/" ? 1 : 0.5,
  }));
}
