"use client";

import { useReverification, useUser } from "@clerk/nextjs";
import {
  Cancel01Icon,
  Mail01Icon,
  PlusSignIcon,
  StarIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import {
  type ClerkUser,
  clerkError,
  type EmailResource,
  Field,
  Section,
  Spinner,
} from "@/components/account/shared";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

const EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/;

type AddState =
  | { step: "idle" }
  | { step: "entering" }
  | { step: "verifying"; pending: EmailResource };

export function EmailSection() {
  const { user } = useUser();
  if (!user) return null;
  return <EmailSectionBody user={user} />;
}

function EmailSectionBody({ user }: { user: ClerkUser }) {
  const createEmail = useReverification((email: string) =>
    user.createEmailAddress({ email }),
  );

  const [add, setAdd] = React.useState<AddState>({ step: "idle" });
  const [email, setEmail] = React.useState("");
  const [code, setCode] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);
  const [rowBusy, setRowBusy] = React.useState<string | null>(null);

  function reset() {
    setAdd({ step: "idle" });
    setEmail("");
    setCode("");
    setError(null);
  }

  async function sendCode() {
    if (!EMAIL_PATTERN.test(email.trim())) {
      setError("Enter a valid email address.");
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const created = await createEmail(email.trim());
      await created.prepareVerification({ strategy: "email_code" });
      setAdd({ step: "verifying", pending: created });
    } catch (err) {
      setError(clerkError(err, "Could not add that email address."));
    } finally {
      setBusy(false);
    }
  }

  async function verifyCode() {
    if (add.step !== "verifying") return;
    setBusy(true);
    setError(null);
    try {
      const verified = await add.pending.attemptVerification({ code });
      if (verified.verification.status !== "verified") {
        setError("That code didn't match. Check your inbox and try again.");
        return;
      }
      await user.reload();
      toast.success(`${verified.emailAddress} verified.`);
      reset();
    } catch (err) {
      setError(
        clerkError(err, "That code didn't match. Check your inbox and retry."),
      );
    } finally {
      setBusy(false);
    }
  }

  async function makePrimary(item: EmailResource) {
    setRowBusy(item.id);
    try {
      await user.update({ primaryEmailAddressId: item.id });
      toast.success(`${item.emailAddress} is now your primary address.`);
    } catch (err) {
      toast.error(clerkError(err, "Could not change your primary address."));
    } finally {
      setRowBusy(null);
    }
  }

  async function remove(item: EmailResource) {
    setRowBusy(item.id);
    try {
      await item.destroy();
      await user.reload();
      toast.success(`${item.emailAddress} removed.`);
    } catch (err) {
      toast.error(clerkError(err, "Could not remove that email address."));
    } finally {
      setRowBusy(null);
    }
  }

  return (
    <Section
      title="Email addresses"
      description="Your primary address receives sign-in codes and account notices."
    >
      <div className="grid gap-3">
        <ul className="grid gap-2">
          {user.emailAddresses.map((item) => {
            const isPrimary = item.id === user.primaryEmailAddressId;
            const verified = item.verification.status === "verified";
            const pending = rowBusy === item.id;
            return (
              <li
                key={item.id}
                className="flex items-center gap-2.5 rounded-lg border border-border/60 px-3 py-2.5"
              >
                <HugeiconsIcon
                  icon={Mail01Icon}
                  strokeWidth={2}
                  className="size-4 shrink-0 text-muted-foreground"
                />
                <span className="min-w-0 flex-1 truncate text-sm">
                  {item.emailAddress}
                </span>
                {isPrimary ? <Badge variant="secondary">Primary</Badge> : null}
                {verified ? null : (
                  <Badge variant="outline" className="text-muted-foreground">
                    Unverified
                  </Badge>
                )}
                {pending ? (
                  <Spinner className="mx-2 text-muted-foreground" />
                ) : null}
                {!isPrimary && verified && !pending ? (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`Make ${item.emailAddress} primary`}
                        onClick={() => makePrimary(item)}
                        className="size-8 text-muted-foreground hover:text-foreground"
                      >
                        <HugeiconsIcon
                          icon={StarIcon}
                          strokeWidth={2}
                          className="size-4"
                        />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Make primary</TooltipContent>
                  </Tooltip>
                ) : null}
                {!isPrimary && !pending ? (
                  <Tooltip>
                    <TooltipTrigger asChild>
                      <Button
                        size="icon"
                        variant="ghost"
                        aria-label={`Remove ${item.emailAddress}`}
                        onClick={() => remove(item)}
                        className="size-8 text-muted-foreground hover:text-destructive"
                      >
                        <HugeiconsIcon
                          icon={Cancel01Icon}
                          strokeWidth={2}
                          className="size-4"
                        />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent>Remove</TooltipContent>
                  </Tooltip>
                ) : null}
              </li>
            );
          })}
        </ul>

        {add.step === "idle" ? (
          <Button
            size="sm"
            variant="outline"
            onClick={() => setAdd({ step: "entering" })}
            className="justify-self-start"
          >
            <HugeiconsIcon
              icon={PlusSignIcon}
              strokeWidth={2}
              className="size-4"
            />
            Add email address
          </Button>
        ) : (
          <div className="grid gap-3 rounded-lg border border-border/60 bg-muted/30 p-3">
            {add.step === "entering" ? (
              <Field
                label="New email address"
                htmlFor="account-new-email"
                error={error}
                hint="We'll send a 6-digit code to confirm it's yours."
              >
                <Input
                  id="account-new-email"
                  type="email"
                  autoComplete="email"
                  placeholder="you@example.com"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter") sendCode();
                  }}
                />
              </Field>
            ) : (
              <Field
                label={`Enter the code sent to ${add.pending.emailAddress}`}
                htmlFor="account-email-code"
                error={error}
                hint="It may take a minute to arrive."
              >
                <Input
                  id="account-email-code"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  maxLength={6}
                  placeholder="000000"
                  value={code}
                  onChange={(e) =>
                    setCode(e.target.value.replace(/\D/g, "").slice(0, 6))
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter") verifyCode();
                  }}
                  className="font-mono tracking-[0.4em]"
                />
              </Field>
            )}

            <div className="flex items-center justify-end gap-2">
              <Button size="sm" variant="ghost" onClick={reset} disabled={busy}>
                Cancel
              </Button>
              {add.step === "entering" ? (
                <Button size="sm" onClick={sendCode} disabled={busy || !email}>
                  {busy ? <Spinner /> : null}
                  Send code
                </Button>
              ) : (
                <Button
                  size="sm"
                  onClick={verifyCode}
                  disabled={busy || code.length < 6}
                >
                  {busy ? <Spinner /> : null}
                  Verify
                </Button>
              )}
            </div>
          </div>
        )}
      </div>
    </Section>
  );
}
