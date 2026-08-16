import {
  SourcePage,
  type SourcePageContent,
  sourceMetadata,
} from "@/components/landing/source-page";

const content: SourcePageContent = {
  slug: "mcp",
  eyebrow: "MCP source",
  title: "Tradstry MCP server",
  summary:
    "Tradstry exposes an authenticated Model Context Protocol server so Claude and other MCP clients can read and write supported parts of a trader's own journal.",
  facts: [
    "Endpoint: https://mcp.tradstry.com/mcp",
    "Authentication: Bearer token through the user's Tradstry account",
    "Tool count: 27 read and write tools",
    "Trading safety: No order placement, withdrawals, or brokerage account control",
  ],
  sections: [
    {
      heading: "What the MCP server is for",
      body: [
        "The MCP server lets an AI client query the user's own Tradstry data through structured tools instead of pasted screenshots or exported spreadsheets. It can read trades, analytics, playbooks, principles, tags, notebook content, workspaces, and media metadata.",
        "The server also offers write tools for supported journal objects such as notes, folders, tags, playbooks, principles, and trade tagging. These writes affect the Tradstry journal, not the connected brokerage account.",
      ],
    },
    {
      heading: "How to connect",
      body: [
        "Use the remote MCP endpoint https://mcp.tradstry.com/mcp in an MCP-compatible client and authenticate with the same Tradstry account that owns the journal data.",
        "The server returns OAuth protected-resource metadata at https://mcp.tradstry.com/.well-known/oauth-protected-resource/mcp so clients can discover the authorization server.",
      ],
    },
    {
      heading: "Boundaries",
      body: [
        "Tradstry's MCP server is a journal and analytics interface. It cannot place trades, move money, modify broker credentials, or bypass the user's account permissions.",
        "Model output can be wrong or incomplete. Tradstry is a record-keeping and review tool, so the trader remains responsible for all trading decisions.",
      ],
    },
  ],
};

export const metadata = sourceMetadata(content);

export default function McpPage() {
  return <SourcePage content={content} />;
}
