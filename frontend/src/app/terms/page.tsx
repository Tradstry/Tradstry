import type { Metadata } from "next";
import {
  Blank,
  Contact,
  LEGAL,
  LegalPage,
  type LegalSection,
} from "@/components/legal/legal-page";

export const metadata: Metadata = {
  title: "Terms of Service",
  description: "The agreement between you and Tradstry.",
  alternates: { canonical: "/terms" },
};

const SECTIONS: LegalSection[] = [
  {
    id: "agreement",
    heading: "The agreement",
    body: (
      <>
        <p>
          These terms are a contract between you and{" "}
          <Blank>{LEGAL.entity}</Blank> ("Tradstry", "we", "us"). They apply the
          moment you create an account or use the service, including the web
          app, the desktop app, and the MCP server.
        </p>
        <p>
          If you don't agree with them, don't use Tradstry. If you're using
          Tradstry on behalf of a company, you're confirming you have the
          authority to bind that company to these terms.
        </p>
      </>
    ),
  },
  {
    id: "not-advice",
    heading: "Tradstry is not financial advice",
    body: (
      <>
        <p>
          <strong>
            Nothing Tradstry produces is a recommendation to buy or sell
            anything.
          </strong>{" "}
          It is a record-keeping and analysis tool. The statistics, the
          playbooks, the principle scoring and anything an AI model says about
          your trades are descriptions of your own past behaviour — not
          predictions and not guidance.
        </p>
        <p>
          Trading involves substantial risk of loss. Past performance does not
          indicate future results. You make your own decisions and you bear
          their consequences. We are not a broker, not an investment adviser,
          and not a fiduciary.
        </p>
      </>
    ),
  },
  {
    id: "account",
    heading: "Your account",
    body: (
      <>
        <p>
          Accounts are handled by Clerk, our authentication provider. You are
          responsible for keeping your credentials safe and for everything that
          happens under your account. Tell us promptly if you think it's been
          compromised.
        </p>
        <p>
          You must be old enough to enter a binding contract where you live, and
          you may not use Tradstry if we're barred from providing it to you
          under applicable law or sanctions.
        </p>
      </>
    ),
  },
  {
    id: "brokerage",
    heading: "Connecting a brokerage",
    body: (
      <>
        <p>
          Brokerage connections run through SnapTrade. When you connect an
          account you authorise us to read your trading activity — executions,
          positions and balances — through them. We request read access; we
          cannot place, modify or cancel orders on your behalf.
        </p>
        <p>
          Broker data arrives as your broker sends it. Fills can be delayed,
          restated, or reported in ways we can't fully reconcile, and some
          brokers don't report cash movements at all. We rebuild your equity
          curve and P&amp;L as faithfully as the underlying data allows, but{" "}
          <strong>
            your broker's own statements are the authoritative record
          </strong>{" "}
          — not ours. Don't use Tradstry as your source of truth for tax or
          compliance filings.
        </p>
        <p>
          You can disconnect a brokerage at any time. Doing so stops future
          syncing; it does not delete what has already been imported.
        </p>
      </>
    ),
  },
  {
    id: "ai",
    heading: "AI features and MCP",
    body: (
      <>
        <p>
          Tradstry exposes your journal to AI models in two ways: the in-app
          assistant, and an MCP server you can connect to a client such as
          Claude. In both cases, the model reads data you have stored with us in
          order to answer you.
        </p>
        <p>
          When you connect the MCP server to your own AI client, that client and
          its provider become responsible for what they do with the data they
          read. Their terms and privacy policy govern that relationship, not
          ours. Granting an AI client write access means it can create and
          modify journal entries, notes, playbooks and tags on your behalf.
        </p>
        <p>
          Model output can be wrong, incomplete, or confidently mistaken. Check
          anything that matters before you act on it.
        </p>
      </>
    ),
  },
  {
    id: "acceptable-use",
    heading: "Acceptable use",
    body: (
      <>
        <p>You agree not to:</p>
        <ul>
          <li>
            Access another person's account or data, or attempt to bypass
            authentication or rate limits.
          </li>
          <li>
            Scrape, resell, or redistribute the service or the data we return.
          </li>
          <li>
            Upload content that is unlawful, or that you don't have the right to
            upload.
          </li>
          <li>
            Interfere with the service's operation, probe it for vulnerabilities
            without permission, or use it to attack anyone else.
          </li>
        </ul>
        <p>
          We may suspend or terminate an account that breaks these rules, and we
          will do so immediately where the breach puts other users at risk.
        </p>
      </>
    ),
  },
  {
    id: "your-content",
    heading: "Your content stays yours",
    body: (
      <>
        <p>
          Your trades, notes, playbooks, images and everything else you put into
          Tradstry belong to you. We claim no ownership of them.
        </p>
        <p>
          You grant us only the narrow licence we need to run the service: to
          store your content, transmit it, back it up, render it in the app, and
          hand it to an AI model when you ask us to.{" "}
          <strong>
            We do not use your content to train AI models, and we do not sell
            it.
          </strong>{" "}
          That licence ends when you delete the content or your account.
        </p>
      </>
    ),
  },
  {
    id: "billing",
    heading: "Subscriptions and billing",
    body: (
      <>
        <p>
          Paid plans renew automatically at the end of each billing period until
          you cancel. Prices are shown before you buy, exclusive of any tax we
          are required to collect.
        </p>
        <p>
          You can cancel at any time from your account settings. Cancellation
          stops the next renewal — you keep access for the rest of the period
          you have already paid for. We don't pro-rate refunds for partial
          periods unless the law where you live requires it.
        </p>
        <p>
          If we change the price, we'll tell you before the change takes effect
          and you'll have the chance to cancel before it applies.
        </p>
      </>
    ),
  },
  {
    id: "availability",
    heading: "Availability",
    body: (
      <p>
        We work hard to keep Tradstry up, but we don't promise uninterrupted
        service. We may take it down for maintenance, and features may change or
        be withdrawn. Parts of Tradstry depend on third parties — Clerk,
        SnapTrade, your broker, market-data providers, AI model providers — and
        when they fail, we may fail with them.
      </p>
    ),
  },
  {
    id: "warranty",
    heading: "Disclaimer of warranties",
    body: (
      <p>
        Tradstry is provided "as is" and "as available", without warranties of
        any kind, express or implied, including merchantability, fitness for a
        particular purpose, and non-infringement. We do not warrant that the
        service will be error-free, that its calculations will be accurate, or
        that it will meet your requirements.
      </p>
    ),
  },
  {
    id: "liability",
    heading: "Limitation of liability",
    body: (
      <>
        <p>
          To the fullest extent the law allows, we are not liable for any
          indirect, incidental, special, consequential or punitive damages, nor
          for any <strong>trading losses</strong>, lost profits, or lost data,
          arising from your use of Tradstry.
        </p>
        <p>
          Our total liability for any claim relating to the service is limited
          to the greater of the amount you paid us in the twelve months before
          the claim arose, or one hundred US dollars.
        </p>
        <p>
          Some jurisdictions don't allow these limits. Where that's true, they
          apply to you only as far as the law permits.
        </p>
      </>
    ),
  },
  {
    id: "termination",
    heading: "Ending the agreement",
    body: (
      <p>
        You can delete your account at any time from the account dialog; doing
        so erases your data as described in our{" "}
        <a href="/privacy">Privacy Policy</a>. We may suspend or terminate your
        access if you break these terms, if we're required to by law, or if we
        stop offering the service — in which case we'll give you reasonable
        notice and a way to export your data.
      </p>
    ),
  },
  {
    id: "changes",
    heading: "Changes to these terms",
    body: (
      <p>
        We may update these terms. If a change materially affects your rights,
        we'll tell you before it takes effect — by email or in the app. Carrying
        on using Tradstry after that means you accept the new terms.
      </p>
    ),
  },
  {
    id: "law",
    heading: "Governing law and contact",
    body: (
      <>
        <p>
          These terms are governed by the laws of{" "}
          <Blank>{LEGAL.jurisdiction}</Blank>, without regard to conflict-of-law
          rules. Disputes go to the courts of that jurisdiction.
        </p>
        <p>
          Questions about any of this: <Contact />.
        </p>
      </>
    ),
  },
];

export default function TermsPage() {
  return (
    <LegalPage
      title="Terms of Service"
      summary="The rules of the road. Written to be read, not to be skipped — the sections on trading risk, broker data and liability are the ones that actually matter to you."
      sections={SECTIONS}
    />
  );
}
