import { FAQS, PLAN_INCLUDES } from "@/components/landing/content";
import { SITE_DESCRIPTION, SITE_NAME, SITE_URL } from "@/lib/site";

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
