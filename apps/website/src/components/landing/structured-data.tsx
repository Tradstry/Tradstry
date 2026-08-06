import { FAQS, PLAN_INCLUDES } from "@/components/landing/content";
import { SITE_DESCRIPTION, SITE_NAME, SITE_URL } from "@/lib/site";

/**
 * Offers are written out rather than derived from PLANS: the annual plan displays "$15/mo"
 * for comparability but is charged as $180 once a year, and a price of 15 with a yearly
 * billing period would be a false claim in structured data.
 */
const OFFERS = [
  { name: "Monthly", price: "20", duration: "P1M" },
  { name: "Annual", price: "180", duration: "P1Y" },
];

const softwareApplication = {
  "@type": "SoftwareApplication",
  "@id": `${SITE_URL}/#software`,
  name: SITE_NAME,
  url: SITE_URL,
  description: SITE_DESCRIPTION,
  applicationCategory: "FinanceApplication",
  applicationSubCategory: "Trading journal",
  operatingSystem: "Web, macOS",
  featureList: PLAN_INCLUDES,
  offers: OFFERS.map((offer) => ({
    "@type": "Offer",
    name: `${SITE_NAME} ${offer.name}`,
    price: offer.price,
    priceCurrency: "USD",
    category: "subscription",
    url: `${SITE_URL}/#pricing`,
    priceSpecification: {
      "@type": "UnitPriceSpecification",
      price: offer.price,
      priceCurrency: "USD",
      billingDuration: offer.duration,
    },
  })),
  publisher: { "@id": `${SITE_URL}/#organization` },
};

const organization = {
  "@type": "Organization",
  "@id": `${SITE_URL}/#organization`,
  name: SITE_NAME,
  url: SITE_URL,
  logo: `${SITE_URL}/icon-512.png`,
};

const faqPage = {
  "@type": "FAQPage",
  "@id": `${SITE_URL}/#faq`,
  mainEntity: FAQS.map((item) => ({
    "@type": "Question",
    name: item.q,
    acceptedAnswer: { "@type": "Answer", text: item.a },
  })),
};

const graph = {
  "@context": "https://schema.org",
  "@graph": [organization, softwareApplication, faqPage],
};

export function StructuredData() {
  return (
    <script
      type="application/ld+json"
      // biome-ignore lint/security/noDangerouslySetInnerHtml: JSON-LD has no other injection point, and the payload is build-time constants.
      dangerouslySetInnerHTML={{ __html: JSON.stringify(graph) }}
    />
  );
}
