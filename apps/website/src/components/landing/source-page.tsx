import type { Metadata } from "next";
import { Footer, Header } from "@/components/landing";
import { SITE_NAME, SITE_URL } from "@/lib/site";

type SourceSection = {
  heading: string;
  body: string[];
  items?: string[];
};

export type SourcePageContent = {
  slug: string;
  title: string;
  eyebrow: string;
  summary: string;
  facts: string[];
  sections: SourceSection[];
};

export function sourceMetadata(content: SourcePageContent): Metadata {
  return {
    title: content.title,
    description: content.summary,
    alternates: { canonical: `/${content.slug}` },
    openGraph: {
      title: `${content.title} - ${SITE_NAME}`,
      description: content.summary,
      url: `${SITE_URL}/${content.slug}`,
    },
  };
}

export function SourcePage({ content }: { content: SourcePageContent }) {
  return (
    <div
      data-shell="marketing"
      className="dark min-h-svh bg-[#0A0A0B] antialiased"
    >
      <Header />
      <main className="mx-auto max-w-6xl px-6 py-20 md:py-24">
        <header className="max-w-3xl">
          <p className="flex items-center gap-3 text-[11px] font-medium uppercase tracking-[0.18em] text-zinc-400">
            <span aria-hidden="true" className="h-px w-6 bg-white/25" />
            {content.eyebrow}
          </p>
          <h1 className="mt-3 text-balance text-4xl font-semibold tracking-[-0.02em] text-zinc-50 md:text-5xl">
            {content.title}
          </h1>
          <p className="mt-5 max-w-2xl text-[16px] leading-relaxed text-zinc-400">
            {content.summary}
          </p>
        </header>

        <section className="mt-14 border-y border-white/[0.06] py-6">
          <h2 className="text-sm font-medium uppercase tracking-[0.16em] text-zinc-500">
            Current facts
          </h2>
          <dl className="mt-5 grid gap-4 md:grid-cols-2">
            {content.facts.map((fact) => {
              const [label, value] = fact.split(": ");
              return (
                <div key={fact} className="space-y-1">
                  <dt className="text-xs font-medium uppercase tracking-[0.12em] text-zinc-600">
                    {label}
                  </dt>
                  <dd className="text-sm leading-relaxed text-zinc-300">
                    {value}
                  </dd>
                </div>
              );
            })}
          </dl>
        </section>

        <article className="mt-14 max-w-[72ch] space-y-12">
          {content.sections.map((section, index) => (
            <section key={section.heading} className="scroll-mt-24">
              <h2 className="flex items-baseline gap-3 text-xl font-semibold tracking-[-0.01em] text-zinc-50">
                <span className="font-mono text-sm text-zinc-500 tabular-nums">
                  {String(index + 1).padStart(2, "0")}
                </span>
                {section.heading}
              </h2>
              <div className="mt-4 space-y-4 text-[15px] leading-[1.75] text-zinc-400 [&_strong]:font-medium [&_strong]:text-zinc-200">
                {section.body.map((paragraph) => (
                  <p key={paragraph}>{paragraph}</p>
                ))}
                {section.items ? (
                  <ul className="list-disc space-y-2 pl-5">
                    {section.items.map((item) => (
                      <li key={item} className="pl-1">
                        {item}
                      </li>
                    ))}
                  </ul>
                ) : null}
              </div>
            </section>
          ))}
        </article>
      </main>
      <Footer />
    </div>
  );
}
