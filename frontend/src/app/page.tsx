"use client";

import {
  Cta,
  Faq,
  Footer,
  Header,
  Hero,
  Mcp,
  Pillars,
  Pricing,
  Problem,
  Proof,
} from "@/components/landing";
import { LeakProvider, LeakSection } from "@/components/landing/leak";

export default function Home() {
  return (
    <div
      data-shell="marketing"
      className="dark min-h-svh bg-[#0A0A0B] antialiased"
    >
      <Header />
      <LeakProvider>
        <main>
          <Hero />
          <Problem />
          <LeakSection />
          <Pillars />
          <Mcp />
          <Proof />
          <Pricing />
          <Faq />
          <Cta />
        </main>
      </LeakProvider>
      <Footer />
    </div>
  );
}
