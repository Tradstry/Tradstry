import {
  SourcePage,
  type SourcePageContent,
  sourceMetadata,
} from "@/components/landing/source-page";

const content: SourcePageContent = {
  slug: "brokerage-sync",
  eyebrow: "Brokerage source",
  title: "Brokerage sync in Tradstry",
  summary:
    "Tradstry connects to brokerages through SnapTrade, imports executions, reconstructs round trips, and keeps broker-derived records separate from the trader's own notes and rules.",
  facts: [
    "Provider: SnapTrade",
    "Institutions: 35+ supported institutions",
    "Examples: Charles Schwab, Fidelity, Interactive Brokers, Robinhood, E*TRADE, Webull, Coinbase, and eToro",
    "Manual fallback: Trades can be entered by hand",
  ],
  sections: [
    {
      heading: "What sync imports",
      body: [
        "When a user connects a brokerage account, Tradstry imports execution data such as symbols, quantities, prices, fees, timestamps, positions, balances, and equity history as the broker reports them through SnapTrade.",
        "Tradstry uses that data to match fills into round-trip trades and attach analytics, journal entries, tags, playbooks, and principle checks.",
      ],
    },
    {
      heading: "Sync schedule",
      body: [
        "Tradstry syncs during US market hours on the hour and half hour between 9:00 and 16:00 Eastern, runs a final pass at 16:30, and syncs once over the weekend.",
        "Nothing polls overnight. Broker reports can still be delayed, restated, incomplete, or represented differently from broker to broker.",
      ],
    },
    {
      heading: "Supported instruments",
      body: [
        "Tradstry supports stocks and options. Option trades carry the underlying, call or put, strike, expiration, and contract multiplier, and P&L is computed against the multiplier rather than only the share price.",
        "Futures, forex, and spot crypto are not supported yet as Tradstry journal instruments, even if a connected provider can expose related account data.",
      ],
    },
  ],
};

export const metadata = sourceMetadata(content);

export default function BrokerageSyncPage() {
  return <SourcePage content={content} />;
}
