/** Every unverified claim on the landing page lives here. Replace each TODO before launch. */

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
