import { clerkMiddleware } from "@clerk/nextjs/server";
import { NextResponse } from "next/server";

// Next 16 renamed the `middleware` convention to `proxy`; Clerk deprecated
// `createRouteMatcher` in favour of resource-level auth. These redirects are a UX/perf
// optimization — bounce signed-out users to sign-in and signed-in users off the entry
// pages — NOT the security boundary. Real data protection is enforced by the backend's
// JWT validation, so plain path matching here is sufficient.

const isEntry = (path: string): boolean =>
  path === "/" ||
  path === "/sign-in" ||
  path.startsWith("/sign-in/") ||
  path === "/sign-up" ||
  path.startsWith("/sign-up/");

// The matcher below only exempts asset extensions, and .txt/.xml are not among them —
// without these a crawler fetching /robots.txt gets redirected to /sign-in.
const CRAWLER_PATHS = new Set([
  "/robots.txt",
  "/sitemap.xml",
  "/llms.txt",
  "/opengraph-image",
]);

const SOURCE_PATHS = new Set([
  "/trading-journal",
  "/mcp",
  "/brokerage-sync",
  "/analytics",
  "/security",
]);

const isPublic = (path: string): boolean =>
  isEntry(path) ||
  path === "/terms" ||
  path === "/privacy" ||
  SOURCE_PATHS.has(path) ||
  CRAWLER_PATHS.has(path);

export default clerkMiddleware(async (auth, req) => {
  const { userId } = await auth();
  const { pathname } = req.nextUrl;

  if (userId && isEntry(pathname)) {
    return NextResponse.redirect(new URL("/dashboard", req.url));
  }

  if (!userId && !isPublic(pathname)) {
    return NextResponse.redirect(new URL("/sign-in", req.url));
  }
});

export const config = {
  matcher: [
    "/((?!_next|tsr|[^?]*\\.(?:html?|css|js(?!on)|jpe?g|webp|png|gif|svg|ttf|woff2?|ico|csv|docx?|xlsx?|zip|webmanifest)).*)",
    "/(api|trpc)(.*)",
  ],
};
