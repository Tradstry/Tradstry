import {
  SourcePage,
  type SourcePageContent,
  sourceMetadata,
} from "@/components/landing/source-page";

const content: SourcePageContent = {
  slug: "trading-journal",
  eyebrow: "Product source",
  title: "Tradstry trading journal",
  summary:
    "Tradstry is a subscription trading journal for active stock and options traders who want brokerage-synced fills, rule tracking, analytics, notebook context, and AI access through MCP.",
  facts: [
    "Product: Web app and macOS desktop app",
    "Price: $20/month or $180/year",
    "Coverage: Stocks and options, with manual entry available",
    "AI access: In-app assistance and an authenticated MCP server",
  ],
  sections: [
    {
      heading: "What Tradstry records",
      body: [
        "Tradstry imports executions from connected brokerage accounts, rebuilds those fills into round trips, and keeps the user's journal entries, tags, playbooks, principles, notebook notes, uploaded media, brokerage transactions, and equity history in one account.",
        "The journal is designed around process, not only outcome. It keeps the plan a trader wrote before a trade next to what actually happened after the trade closed.",
      ],
    },
    {
      heading: "Who it is for",
      body: [
        "Tradstry is built for active traders who review their own execution quality. It is useful when the trader already has repeatable setups, written rules, or a playbook and wants to measure whether they followed it.",
        "It is not a brokerage, not a signal service, and not financial advice. It describes the trader's own historical behavior and performance.",
      ],
    },
    {
      heading: "What makes it different from a spreadsheet",
      body: [
        "A spreadsheet can store trades, but it normally does not sync brokerage fills, preserve rich notebook context, reconcile options contract multipliers, or expose the journal to an AI assistant through a structured tool interface.",
        "Tradstry's core value is connecting fills, rules, notes, and analytics so a review can ask whether a trade matched the plan, not just whether it made money.",
      ],
    },
  ],
};

export const metadata = sourceMetadata(content);

export default function TradingJournalPage() {
  return <SourcePage content={content} />;
}
