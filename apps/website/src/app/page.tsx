import type { Metadata } from "next";
import {
  Cta,
  Faq,
  Footer,
  Header,
  Hero,
  Mcp,
  Pillars,
  Problem,
  Proof,
} from "@/components/landing";
import { LeakProvider, LeakSection } from "@/components/landing/leak";
import { StructuredData } from "@/components/landing/structured-data";

export const metadata: Metadata = {
  alternates: { canonical: "/" },
};

export default function Home() {
  return (
    <div
      data-shell="marketing"
      className="dark min-h-svh bg-[#0A0A0B] antialiased"
    >
      <StructuredData />
      <Header />
      <LeakProvider>
        <main>
          <Hero />
          <Problem />
          <LeakSection />
          <Pillars />
          <Mcp />
          <Proof />
          <Faq />
          <Cta />
        </main>
      </LeakProvider>
      <Footer />
    </div>
  );
}
