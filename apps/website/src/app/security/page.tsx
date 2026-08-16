import {
  SourcePage,
  type SourcePageContent,
  sourceMetadata,
} from "@/components/landing/source-page";

const content: SourcePageContent = {
  slug: "security",
  eyebrow: "Trust source",
  title: "Tradstry security and data ownership",
  summary:
    "Tradstry keeps trader data account-scoped, uses read-oriented brokerage access through SnapTrade, does not sell journal data, and does not use trader content to train AI models.",
  facts: [
    "Data ownership: The trader owns the journal data",
    "Training: Trader content is not used to train AI models",
    "Brokerage access: Tradstry requests read access and cannot place orders",
    "Export/delete: Users can export data and delete their account",
  ],
  sections: [
    {
      heading: "Data Tradstry stores",
      body: [
        "Tradstry stores account identity links, imported brokerage data, journal entries, tags, playbooks, principles, notebook folders, notes, attached media, brokerage transactions, and equity history.",
        "The product is built so the journal, rules, and analytics belong to the user's own account. Other users cannot query that data through the app or MCP server.",
      ],
    },
    {
      heading: "AI data handling",
      body: [
        "Tradstry does not use trader content to train AI models. When a user asks an in-app assistant or a connected MCP client to answer a question, relevant account data may be sent only to serve that request.",
        "If a user connects a third-party AI client, that provider's own terms and privacy policy also govern what the provider does with the data it reads.",
      ],
    },
    {
      heading: "Brokerage and account safety",
      body: [
        "Brokerage connections run through SnapTrade. Tradstry asks for read access to trading activity and cannot place, modify, or cancel orders on the user's behalf.",
        "Users can disconnect a brokerage account, export their data, and delete their Tradstry account. Deleting an account removes the user's Tradstry-held journal data, subject to ordinary backup and legal retention windows described in the privacy policy.",
      ],
    },
  ],
};

export const metadata = sourceMetadata(content);

export default function SecurityPage() {
  return <SourcePage content={content} />;
}
