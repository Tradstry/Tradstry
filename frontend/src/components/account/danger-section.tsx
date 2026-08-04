"use client";

import { useClerk, useReverification, useUser } from "@clerk/nextjs";
import { Alert02Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import {
  type ClerkUser,
  clerkError,
  Field,
  Section,
  Spinner,
} from "@/components/account/shared";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { capture, EVENTS } from "@/lib/analytics/events";

const LOSSES = [
  "Every trade, tag and journal entry",
  "Your playbooks, principles and notebook",
  "Brokerage connections and synced history",
];

export function DangerSection() {
  const { user } = useUser();
  if (!user) return null;
  return <DangerSectionBody user={user} />;
}

function DangerSectionBody({ user }: { user: ClerkUser }) {
  const { signOut } = useClerk();
  const deleteAccount = useReverification(() => user.delete());

  const [confirmation, setConfirmation] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const email = user.primaryEmailAddress?.emailAddress ?? "";
  const matches = confirmation.trim().toLowerCase() === email.toLowerCase();

  async function destroy() {
    if (!matches) return;
    setBusy(true);
    setError(null);
    capture(EVENTS.accountDeletionRequested, {});
    try {
      await deleteAccount();
      toast.success("Your account has been deleted.");
      await signOut({ redirectUrl: "/" });
    } catch (err) {
      setError(clerkError(err, "Could not delete your account."));
      setBusy(false);
    }
  }

  return (
    <Section
      tone="destructive"
      title="Delete account"
      description="This permanently erases your account and everything in it. It cannot be undone."
      footer={
        <Button
          size="sm"
          variant="destructive"
          onClick={destroy}
          disabled={!matches || busy}
        >
          {busy ? <Spinner /> : null}
          Delete my account
        </Button>
      }
    >
      <div className="grid gap-4">
        <ul className="grid gap-1.5">
          {LOSSES.map((item) => (
            <li
              key={item}
              className="flex items-center gap-2 text-xs text-muted-foreground"
            >
              <HugeiconsIcon
                icon={Alert02Icon}
                strokeWidth={2}
                className="size-3.5 shrink-0 text-destructive"
              />
              {item}
            </li>
          ))}
        </ul>

        <Field
          label="Type your email to confirm"
          htmlFor="account-delete-confirm"
          error={error}
          hint={email}
        >
          <Input
            id="account-delete-confirm"
            autoComplete="off"
            placeholder={email}
            value={confirmation}
            onChange={(e) => setConfirmation(e.target.value)}
            aria-invalid={confirmation.length > 0 && !matches}
          />
        </Field>
      </div>
    </Section>
  );
}
