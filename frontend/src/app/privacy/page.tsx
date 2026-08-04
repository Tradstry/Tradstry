import type { Metadata } from "next";
import {
  Contact,
  LEGAL,
  LegalPage,
  type LegalSection,
} from "@/components/legal/legal-page";

export const metadata: Metadata = {
  title: "Privacy Policy",
  description: "What Tradstry collects, why, and what it never does.",
  alternates: { canonical: "/privacy" },
};

const SECTIONS: LegalSection[] = [
  {
    id: "summary",
    heading: "The short version",
    body: (
      <>
        <p>
          Your journal is yours. We collect what we need to run the service and
          nothing we don't.
        </p>
        <ul>
          <li>
            <strong>We do not sell your data</strong>, to anyone, ever.
          </li>
          <li>
            <strong>We do not train AI models on your content.</strong>
          </li>
          <li>
            We ask brokers for <strong>read-only</strong> access. We cannot
            trade on your behalf.
          </li>
          <li>Delete your account and your data goes with it.</li>
        </ul>
        <p>The rest of this page is the detail behind those four lines.</p>
      </>
    ),
  },
  {
    id: "collect",
    heading: "What we collect",
    body: (
      <>
        <p>
          <strong>Account details.</strong> Your name, email address, profile
          photo and sign-in method. These are held by Clerk, our authentication
          provider; we store only the identifier that links you to your data.
        </p>
        <p>
          <strong>Trading data.</strong> When you connect a brokerage, we import
          your executions, positions and balances through SnapTrade — trade
          times, symbols, quantities, prices, fees and account values.
        </p>
        <p>
          <strong>What you write.</strong> Journal entries, tags, playbooks,
          principles, notebook pages, and any images or files you attach.
        </p>
        <p>
          <strong>Operational data.</strong> Logs needed to keep the service
          running and secure: IP address, browser and device type, timestamps,
          and error traces. Session and device information is held by Clerk so
          you can see and revoke your own sessions.
        </p>
        <p>
          We don't ask for, and don't want, your brokerage password. SnapTrade
          handles that connection.
        </p>
      </>
    ),
  },
  {
    id: "why",
    heading: "Why we hold it",
    body: (
      <ul>
        <li>To show you your own trades, analytics and notes.</li>
        <li>To sync your data between the web app and the desktop app.</li>
        <li>
          To answer questions you ask an AI model about your journal, whether in
          the app or over MCP.
        </li>
        <li>To take payment and manage your subscription.</li>
        <li>To keep the service secure, debug failures, and prevent abuse.</li>
        <li>To email you about your account when we need to.</li>
      </ul>
    ),
  },
  {
    id: "analytics",
    heading: "Product analytics",
    body: (
      <>
        <p>
          We use Countly to understand how Tradstry is used. It receives product
          usage data so we can improve the app.
        </p>
        <p>What it records:</p>
        <ul>
          <li>Pages you visit and features you use, tied to your account.</li>
          <li>Error reports from the web app, used to find bugs.</li>
        </ul>
        <p>What is never recorded:</p>
        <ul>
          <li>Your P&amp;L figures, account balances and trade values.</li>
          <li>Anything you type into a form field.</li>
          <li>The contents of your notebook and journal entries.</li>
        </ul>
        <p>
          Analytics data is deleted along with the rest of your data when you
          delete your account.
        </p>
      </>
    ),
  },
  {
    id: "ai",
    heading: "AI, and what it sees",
    body: (
      <>
        <p>
          When you use the in-app assistant, the relevant parts of your journal
          are sent to a model provider so it can answer. When you connect the
          MCP server to your own AI client — Claude, for instance — that client
          reads your data directly, under your own subscription with that
          provider.
        </p>
        <p>
          <strong>
            Your content is not used to train any model, ours or a provider's.
          </strong>{" "}
          We send data to model providers only to serve the request you made.
        </p>
        <p>
          Once you point a third-party AI client at your journal, that
          provider's own privacy policy governs what they do with what they
          read. That's a relationship between you and them; choose your client
          accordingly.
        </p>
      </>
    ),
  },
  {
    id: "processors",
    heading: "Who else touches it",
    body: (
      <>
        <p>
          We use a small number of processors, each for one job, each bound to
          use your data only to do that job:
        </p>
        <ul>
          <li>
            <strong>Clerk</strong> — authentication, sessions, profile photos.
          </li>
          <li>
            <strong>SnapTrade</strong> — the brokerage connection.
          </li>
          <li>
            <strong>Vercel</strong> — serving the web app itself.
          </li>
          <li>
            <strong>Contabo</strong> — the servers our database runs on.
          </li>
          <li>
            <strong>AI model providers</strong> — answering the questions you
            ask.
          </li>
          <li>
            <strong>Market-data providers</strong> — historical prices used to
            reconstruct your equity curve. We send them ticker symbols, never
            anything about you.
          </li>
        </ul>
        <p>
          Your journal itself lives in a database we run on our own servers, not
          in a third party's product.{" "}
          <strong>Tradstry is the data controller</strong> — the decisions about
          what is collected and why are ours, and the responsibility is ours. We
          do not share your journal with advertisers, data brokers, or anyone
          else. We will disclose data if the law compels us to — and where we're
          allowed to tell you, we will.
        </p>
      </>
    ),
  },
  {
    id: "retention",
    heading: "How long we keep it",
    body: (
      <>
        <p>
          We keep your data for as long as your account exists. Delete something
          and it goes; delete your account and everything goes with it.
        </p>
        <p>
          Two honest caveats. Encrypted backups roll off on their own schedule,
          so deleted data can persist there for a short window before it's
          overwritten. And where the law requires us to retain records —
          billing, for instance — we retain those and nothing more.
        </p>
      </>
    ),
  },
  {
    id: "rights",
    heading: "Your rights",
    body: (
      <>
        <p>
          You can see, correct, export and delete your data. Most of it you can
          do yourself: the account dialog handles your profile, email addresses,
          devices and account deletion. For anything else, write to us and we'll
          do it.
        </p>
        <p>
          Depending on where you live, you may also have the right to object to
          or restrict certain processing, and to complain to a data-protection
          authority. Exercising any of these rights costs you nothing and we
          won't treat you differently for it.
        </p>
      </>
    ),
  },
  {
    id: "security",
    heading: "Security",
    body: (
      <p>
        Data is encrypted in transit. Access to production systems is limited to
        the people who need it. Sessions can be revoked per device from your
        account settings. No system is perfectly secure, and we won't pretend
        otherwise — but if a breach ever affects your data, we'll tell you
        promptly and plainly.
      </p>
    ),
  },
  {
    id: "california",
    heading: "California residents",
    body: (
      <>
        <p>
          If you live in California, the CCPA gives you the right to know what
          personal information we collect and why, to get a copy of it, to
          correct it, to delete it, and not to be discriminated against for
          asking. The sections above describe all of that, and the account
          dialog lets you do most of it yourself.
        </p>
        <p>
          <strong>
            We do not sell your personal information, and we do not share it for
            cross-context behavioural advertising.
          </strong>{" "}
          We never have. There is therefore nothing for you to opt out of — but
          if that ever changes, we will tell you before it does, and the opt-out
          will be here.
        </p>
      </>
    ),
  },
  {
    id: "children",
    heading: "Children",
    body: (
      <p>
        Tradstry isn't for anyone under 18. We don't knowingly collect data from
        children, and we'll delete it if we discover we have.
      </p>
    ),
  },
  {
    id: "changes",
    heading: "Changes, and how to reach us",
    body: (
      <>
        <p>
          If we change this policy in a way that materially affects you, we'll
          tell you before it takes effect rather than quietly editing the page.
        </p>
        <p>
          Questions, requests, or complaints: <Contact />. The data controller
          is {LEGAL.entity}.
        </p>
      </>
    ),
  },
];

export default function PrivacyPage() {
  return (
    <LegalPage
      title="Privacy Policy"
      summary="What we collect, why we hold it, who else sees it, and how to get rid of it. Written in plain sentences, because a privacy policy nobody reads protects nobody."
      sections={SECTIONS}
    />
  );
}
