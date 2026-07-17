import { SignUp } from "@clerk/nextjs";
import type { Metadata } from "next";
import { inkAppearance } from "@/components/auth/appearance";
import { AuthShell } from "@/components/auth/auth-shell";

export const metadata: Metadata = {
  title: "Start journalling · Tradstry",
};

export default function SignUpPage() {
  return (
    <AuthShell
      title="Start the record."
      subtitle="Connect a broker and it fills itself."
    >
      <SignUp appearance={inkAppearance} />
    </AuthShell>
  );
}
