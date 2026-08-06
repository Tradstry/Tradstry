/**
 * One source of truth for every machine-readable surface: metadata, robots.txt,
 * sitemap.xml, JSON-LD and llms.txt all read from here so they can never disagree.
 */

export const SITE_URL = "https://tradstry.com";

export const SITE_NAME = "Tradstry";

/** What an AI assistant quotes when asked what Tradstry is — so it carries the specifics. */
export const SITE_DESCRIPTION =
  "Tradstry is a trading journal that syncs every fill from 35+ brokerages, holds you to the rules you wrote, and computes 36 performance analytics. Query and edit your journal from Claude over MCP. $20/month or $180/year, with a macOS desktop app.";

/**
 * Bumped by hand when public page copy changes. A build-time `new Date()` would mark
 * every route as freshly modified on every deploy, which is a recrawl signal crawlers
 * learn to ignore.
 */
export const CONTENT_LAST_MODIFIED = new Date("2026-07-30");

export const PUBLIC_ROUTES = ["/", "/privacy", "/terms"] as const;

/** Routes that exist only behind auth — kept out of the sitemap and disallowed in robots.txt. */
export const PRIVATE_PATHS = ["/dashboard/", "/api/", "/sign-in/", "/sign-up/"];
