import {
  SourcePage,
  type SourcePageContent,
  sourceMetadata,
} from "@/components/landing/source-page";

const content: SourcePageContent = {
  slug: "analytics",
  eyebrow: "Analytics source",
  title: "Tradstry trading analytics",
  summary:
    "Tradstry computes 36 trading analytics across overview, risk and reward, edge, and behavior views so traders can compare outcomes against the rules they meant to follow.",
  facts: [
    "Metric count: 36 computed measures",
    "Views: Overview, Risk & Reward, Edges, and Behavior",
    "Examples: Expectancy, SQN, maximum drawdown, R-multiple distribution, win rate, average win, and average loss",
    "Discipline split: Clean trades versus trades that broke a rule",
  ],
  sections: [
    {
      heading: "What the analytics measure",
      body: [
        "Tradstry measures both performance and behavior. It computes outcome metrics such as P&L, win rate, expectancy, profit factor, drawdown, average win, average loss, and R-multiple distribution.",
        "It also separates trades that followed the trader's written playbook or principles from trades that violated those rules, so the trader can evaluate execution quality instead of only account outcome.",
      ],
    },
    {
      heading: "How the numbers are used",
      body: [
        "Analytics are attached to the trader's own imported or manually entered trades. They are intended for review, journaling, and process improvement.",
        "The figures are not predictions, recommendations, or guarantees. Broker statements remain the authoritative record for account, tax, and compliance purposes.",
      ],
    },
    {
      heading: "How AI can access analytics",
      body: [
        "Tradstry's MCP server includes tools for calculating analytics and querying the journal, which lets a connected AI client answer questions about setups, tags, playbooks, and historical trading behavior.",
        "The model works from the user's available Tradstry data and should say when a requested answer needs data that is missing or outside the account's scope.",
      ],
    },
  ],
};

export const metadata = sourceMetadata(content);

export default function AnalyticsPage() {
  return <SourcePage content={content} />;
}
