import { SignIn } from "@clerk/nextjs";
import type { Metadata } from "next";
import { inkAppearance } from "@/components/auth/appearance";
import { AuthShell } from "@/components/auth/auth-shell";

export const metadata: Metadata = {
  title: "Sign in · Tradstry",
  robots: {
    index: false,
    follow: false,
  },
};

export default function SignInPage() {
  return (
    <AuthShell
      title="Welcome back."
      subtitle="The record kept going without you."
    >
      <SignIn appearance={inkAppearance} />
    </AuthShell>
  );
}
