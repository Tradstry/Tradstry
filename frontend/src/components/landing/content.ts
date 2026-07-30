/** Every unverified claim on the landing page lives here. Replace each TODO before launch. */

/** Rendered by <Faq> and emitted as FAQPage JSON-LD, so both always say the same thing. */
export const FAQS = [
  {
    q: "Which brokers can I connect?",
    a: "Anything SnapTrade supports — 35+ institutions including Schwab, Fidelity, Interactive Brokers, Robinhood, E*TRADE, Webull, Coinbase and eToro. Tradstry pulls your executions and matches them into round trips. You can also add trades by hand if your broker isn't covered.",
  },
  {
    q: "What is MCP, in one sentence?",
    a: "It's the protocol Claude uses to talk to outside tools. Point Claude at Tradstry's MCP server and it can read your trades, analytics, playbooks and notebook — and write to them — without you pasting anything.",
  },
  {
    q: "Does the AI cost extra?",
    a: "No. Over MCP you use your own Claude subscription and your own model, so you're not paying us a margin on tokens.",
  },
  {
    q: "Does it work offline?",
    a: "The desktop app does. It keeps a local database, lets you journal on a plane, and merges cleanly when you reconnect — no last-write-wins data loss.",
  },
  {
    q: "Who owns my data?",
    a: "You do. Export it whenever you like, and deleting your account erases it. We don't sell it, and we don't train on it.",
  },
  {
    q: "Can I cancel?",
    a: "Any time, from the account dialog. You keep access until the end of the period you paid for.",
  },
  {
    q: "Which instruments can I journal?",
    a: "Stocks and options. Option trades carry the underlying, call or put, strike, expiration and contract multiplier, and P&L is computed against the multiplier rather than the share price. Futures, forex and spot crypto are not supported yet.",
  },
  {
    q: "How often does Tradstry sync with my broker?",
    a: "Every half hour during US market hours — on the hour and the half hour between 9:00 and 16:00 Eastern — plus a final pass at 16:30 to catch the close, and once over the weekend. Nothing polls overnight, so a sync is never more than thirty minutes behind the market.",
  },
  {
    q: "Is there a Windows or Linux desktop app?",
    a: "Not today. The desktop app is macOS only. Tradstry runs in any modern browser on Windows and Linux, and the browser version has everything except the offline local database.",
  },
  {
    q: "Is there a mobile app?",
    a: "No. Tradstry is responsive and readable on a phone browser, but there is no iOS or Android app, and journalling is designed around a keyboard.",
  },
  {
    q: "Can I share my journal with a coach or a team?",
    a: "No. Tradstry is one account, one trader. There are no seats, roles or shared workspaces — the sync that exists is between your own devices, not between people.",
  },
  {
    q: "What exactly do I get if I export my data?",
    a: "One JSON file containing every row Tradstry holds for you — trades, journal entries, playbooks, principles, tags, notebook folders and notes, brokerage transactions and equity history — plus seven-day download links for any images you uploaded. It downloads from the account page, and exporting deletes nothing.",
  },
  {
    q: "Which account currencies are supported?",
    a: "USD, EUR, GBP, JPY, CAD, AUD and CHF. Each trading account carries its own currency, so a multi-currency book stays separated rather than being converted into one base number.",
  },
];

export type Metric = { value: string; label: string; note: string };
export type Testimonial = { quote: string; name: string; role: string };
export type Plan = {
  id: "monthly" | "annual";
  price: string;
  cadence: string;
  note: string;
  cta: string;
};

export const PLACEHOLDER = "TODO";

/**
 * Product facts, not adoption claims — every one is checkable.
 * 27 = tools in mcp-server/src/tools; 36 = ANALYTICS_SECTIONS + ADVANCED_SECTIONS.
 */
export const METRICS: Metric[] = [
  {
    value: "27",
    label: "MCP tools",
    note: "Read and write your journal from Claude",
  },
  {
    value: "36",
    label: "Analytics computed",
    note: "Expectancy, SQN, drawdown, R-distribution",
  },
  {
    // SnapTrade's own published figure: "400M+ retail investor accounts across 35+
    // financial institutions". We set no brokerage_id filter, so we inherit all of them.
    value: "35+",
    label: "Brokerages supported",
    note: "Schwab, Fidelity, IBKR, Robinhood and more, via SnapTrade",
  },
  {
    value: "0",
    label: "Of your data used for training",
    note: "Export it or delete it, any time",
  },
];

/**
 * Layout placeholders, deliberately NOT written as plausible reviews.
 * A testimonial from a person who does not exist is illegal under the FTC's rule on
 * consumer reviews (16 CFR 465), so these must read as obviously unfilled until real
 * quotes replace them — swap all three fields per entry and the chips disappear.
 */
export const TESTIMONIALS: Testimonial[] = [
  {
    quote: PLACEHOLDER,
    name: PLACEHOLDER,
    role: PLACEHOLDER,
  },
  {
    quote: PLACEHOLDER,
    name: PLACEHOLDER,
    role: PLACEHOLDER,
  },
  {
    quote: PLACEHOLDER,
    name: PLACEHOLDER,
    role: PLACEHOLDER,
  },
];

/** What each placeholder is asking you to collect, shown in the empty card. */
export const TESTIMONIAL_PROMPTS = [
  "What did the journal show them that they could not see before?",
  "Which rule were they breaking, and what did it cost?",
  "What do they do now that they did not do a year ago?",
];

/**
 * A worked example, not anyone's trading record — and the copy says so on the page.
 * Figures are the demo book shown in this page's screenshots, so the two agree:
 * 138 trades, 110 clean / 28 breaking a principle.
 */
export const EXAMPLE = {
  lede: "The rules are the edge. Here is the size of it.",
  body: "One account, one year, 138 trades. Same trader, same setups — split by whether the trade followed the plan that was written before it was taken. Tradstry is what makes that split visible.",
  columns: [
    {
      title: "Followed the plan",
      count: "110 trades",
      tone: "profit",
      rows: [
        { label: "Win rate", value: "70.9%" },
        { label: "Average loss", value: "−0.97R" },
      ],
    },
    {
      title: "Broke a rule",
      count: "28 trades",
      tone: "loss",
      rows: [
        { label: "Win rate", value: "14.3%" },
        { label: "Average loss", value: "−2.45R" },
      ],
    },
  ],
  footnote:
    "Example account — the same book shown in the screenshots on this page. Illustrative figures, not a projection and not a performance claim.",
} as const;

export const PLANS: Plan[] = [
  {
    id: "monthly",
    price: "$20",
    cadence: "/mo",
    note: "Billed monthly · Cancel anytime",
    cta: "Start monthly",
  },
  {
    id: "annual",
    price: "$15",
    cadence: "/mo",
    // $240 monthly − $180 annual = $60, which is exactly three months at $20.
    note: "$180 billed annually · three months free",
    cta: "Start annual",
  },
];

export const PLAN_INCLUDES = [
  "Unlimited trades, tags and journal entries",
  "Brokerage sync with automatic trade matching",
  "Full analytics suite — expectancy, SQN, drawdown, R-distribution",
  "Playbooks, principles and discipline tracking",
  "Notebook with offline-first sync",
  "Desktop app for macOS",
  "MCP server — bring your journal into Claude",
];

export const SCREENSHOTS = {
  workspace: {
    src: "/shot-dashboard.png",
    alt: "The Tradstry dashboard",
    ratio: "2992 / 1716",
  },
  journal: {
    src: "/shot-journal.png",
    alt: "The trade journal, with each trade tagged by setup and mistake",
    ratio: "2992 / 1719",
  },
  notebook: {
    src: "/shot-notebook.png",
    alt: "The notebook, with a trading lesson written as a numbered list",
    ratio: "2992 / 1713",
  },
  mcp: {
    src: "/shot-mcp.png",
    alt: "Claude calling the Tradstry MCP server to analyse a trading account",
    ratio: "1782 / 1578",
  },
} satisfies Record<string, { src: string | null; alt: string; ratio: string }>;
