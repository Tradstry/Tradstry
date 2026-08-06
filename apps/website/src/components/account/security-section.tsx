"use client";

import { useReverification, useSession, useUser } from "@clerk/nextjs";
import {
  Cancel01Icon,
  ComputerIcon,
  Link01Icon,
  SmartPhone01Icon,
  ViewIcon,
  ViewOffSlashIcon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import {
  type ClerkUser,
  clerkError,
  type DeviceSession,
  EmptyRow,
  type ExternalAccount,
  Field,
  Section,
  Spinner,
} from "@/components/account/shared";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";

const MIN_PASSWORD_LENGTH = 8;

const PROVIDERS = [
  { strategy: "oauth_google", id: "google", label: "Google" },
  { strategy: "oauth_apple", id: "apple", label: "Apple" },
  { strategy: "oauth_github", id: "github", label: "GitHub" },
] as const;

export function SecuritySection() {
  const { user } = useUser();
  if (!user) return null;
  return (
    <div className="grid gap-4">
      <PasswordCard user={user} />
      <ConnectionsCard user={user} />
      <DevicesCard user={user} />
    </div>
  );
}

function PasswordCard({ user }: { user: ClerkUser }) {
  const changePassword = useReverification(
    (params: {
      currentPassword?: string;
      newPassword: string;
      signOutOfOtherSessions: boolean;
    }) => user.updatePassword(params),
  );

  const hasPassword = user.passwordEnabled;
  const [current, setCurrent] = React.useState("");
  const [next, setNext] = React.useState("");
  const [confirm, setConfirm] = React.useState("");
  const [signOutOthers, setSignOutOthers] = React.useState(true);
  const [reveal, setReveal] = React.useState(false);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const ready =
    next.length >= MIN_PASSWORD_LENGTH &&
    next === confirm &&
    (!hasPassword || current.length > 0);

  async function submit() {
    if (next !== confirm) {
      setError("The two passwords don't match.");
      return;
    }
    if (next.length < MIN_PASSWORD_LENGTH) {
      setError(`Use at least ${MIN_PASSWORD_LENGTH} characters.`);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await changePassword({
        ...(hasPassword ? { currentPassword: current } : {}),
        newPassword: next,
        signOutOfOtherSessions: signOutOthers,
      });
      setCurrent("");
      setNext("");
      setConfirm("");
      toast.success(hasPassword ? "Password changed." : "Password set.");
    } catch (err) {
      setError(clerkError(err, "Could not update your password."));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Section
      title={hasPassword ? "Password" : "Set a password"}
      description={
        hasPassword
          ? "Changing it can sign you out everywhere else."
          : "You currently sign in without a password. Set one as a fallback."
      }
      footer={
        <Button size="sm" onClick={submit} disabled={!ready || busy}>
          {busy ? <Spinner /> : null}
          {hasPassword ? "Change password" : "Set password"}
        </Button>
      }
    >
      <div className="grid gap-4">
        {hasPassword ? (
          <Field label="Current password" htmlFor="account-current-password">
            <Input
              id="account-current-password"
              type="password"
              autoComplete="current-password"
              value={current}
              onChange={(e) => setCurrent(e.target.value)}
            />
          </Field>
        ) : null}

        <div className="grid gap-4 sm:grid-cols-2">
          <Field
            label="New password"
            htmlFor="account-new-password"
            hint={`At least ${MIN_PASSWORD_LENGTH} characters.`}
          >
            <div className="relative">
              <Input
                id="account-new-password"
                type={reveal ? "text" : "password"}
                autoComplete="new-password"
                value={next}
                onChange={(e) => setNext(e.target.value)}
                className="pr-9"
              />
              <button
                type="button"
                aria-label={reveal ? "Hide password" : "Show password"}
                onClick={() => setReveal((v) => !v)}
                className="absolute inset-y-0 right-0 flex w-9 items-center justify-center rounded-r-md text-muted-foreground outline-none transition-colors hover:text-foreground focus-visible:ring-2 focus-visible:ring-blue-500/70"
              >
                <HugeiconsIcon
                  icon={reveal ? ViewOffSlashIcon : ViewIcon}
                  strokeWidth={2}
                  className="size-4"
                />
              </button>
            </div>
          </Field>
          <Field
            label="Confirm new password"
            htmlFor="account-confirm-password"
            error={
              confirm.length > 0 && confirm !== next
                ? "Passwords don't match."
                : null
            }
          >
            <Input
              id="account-confirm-password"
              type={reveal ? "text" : "password"}
              autoComplete="new-password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
            />
          </Field>
        </div>

        <div className="flex items-center gap-2">
          <Checkbox
            id="account-signout-others"
            checked={signOutOthers}
            onCheckedChange={(v) => setSignOutOthers(v === true)}
          />
          <Label
            htmlFor="account-signout-others"
            className="text-xs font-normal text-muted-foreground"
          >
            Sign out of all other devices
          </Label>
        </div>

        {error ? (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        ) : null}
      </div>
    </Section>
  );
}

function ConnectionsCard({ user }: { user: ClerkUser }) {
  const [busy, setBusy] = React.useState<string | null>(null);
  const connected = new Set(user.externalAccounts.map((a) => a.provider));
  const available = PROVIDERS.filter((p) => !connected.has(p.id));

  async function connect(strategy: (typeof PROVIDERS)[number]["strategy"]) {
    setBusy(strategy);
    try {
      const account = await user.createExternalAccount({
        strategy,
        redirectUrl: window.location.href,
      });
      const url = account.verification?.externalVerificationRedirectURL;
      if (!url) throw new Error("no redirect");
      window.location.href = url.toString();
    } catch (err) {
      toast.error(clerkError(err, "Could not start that connection."));
      setBusy(null);
    }
  }

  async function disconnect(account: ExternalAccount) {
    setBusy(account.id);
    try {
      await account.destroy();
      await user.reload();
      toast.success("Account disconnected.");
    } catch (err) {
      toast.error(clerkError(err, "Could not disconnect that account."));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Section
      title="Connected accounts"
      description="Sign in to Tradstry with a provider you already trust."
    >
      <div className="grid gap-3">
        {user.externalAccounts.length === 0 ? (
          <EmptyRow>No providers connected yet.</EmptyRow>
        ) : (
          <ul className="grid gap-2">
            {user.externalAccounts.map((account) => {
              const label =
                PROVIDERS.find((p) => p.id === account.provider)?.label ??
                account.provider;
              const pending = busy === account.id;
              return (
                <li
                  key={account.id}
                  className="flex items-center gap-2.5 rounded-lg border border-border/60 px-3 py-2.5"
                >
                  <HugeiconsIcon
                    icon={Link01Icon}
                    strokeWidth={2}
                    className="size-4 shrink-0 text-muted-foreground"
                  />
                  <span className="text-sm font-medium">{label}</span>
                  <span className="min-w-0 flex-1 truncate text-xs text-muted-foreground">
                    {account.emailAddress || account.username}
                  </span>
                  {account.verification?.status === "verified" ? null : (
                    <Badge variant="outline" className="text-muted-foreground">
                      Needs reconnect
                    </Badge>
                  )}
                  {pending ? (
                    <Spinner className="mx-2 text-muted-foreground" />
                  ) : (
                    <Tooltip>
                      <TooltipTrigger asChild>
                        <Button
                          size="icon"
                          variant="ghost"
                          aria-label={`Disconnect ${label}`}
                          onClick={() => disconnect(account)}
                          className="size-8 text-muted-foreground hover:text-destructive"
                        >
                          <HugeiconsIcon
                            icon={Cancel01Icon}
                            strokeWidth={2}
                            className="size-4"
                          />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent>Disconnect</TooltipContent>
                    </Tooltip>
                  )}
                </li>
              );
            })}
          </ul>
        )}

        {available.length > 0 ? (
          <div className="flex flex-wrap gap-2">
            {available.map((provider) => (
              <Button
                key={provider.id}
                size="sm"
                variant="outline"
                disabled={busy !== null}
                onClick={() => connect(provider.strategy)}
              >
                {busy === provider.strategy ? <Spinner /> : null}
                Connect {provider.label}
              </Button>
            ))}
          </div>
        ) : null}
      </div>
    </Section>
  );
}

function deviceLabel(session: DeviceSession): string {
  const { browserName, deviceType, isMobile } = session.latestActivity;
  const device = deviceType ?? (isMobile ? "Mobile device" : "Desktop");
  return browserName ? `${browserName} on ${device}` : device;
}

function deviceLocation(session: DeviceSession): string {
  const { city, country, ipAddress } = session.latestActivity;
  return [city, country].filter(Boolean).join(", ") || (ipAddress ?? "Unknown");
}

const RELATIVE = new Intl.RelativeTimeFormat("en", { numeric: "auto" });

function lastActive(date: Date): string {
  const minutes = Math.round((date.getTime() - Date.now()) / 60_000);
  if (Math.abs(minutes) < 60) return RELATIVE.format(minutes, "minute");
  const hours = Math.round(minutes / 60);
  if (Math.abs(hours) < 24) return RELATIVE.format(hours, "hour");
  return RELATIVE.format(Math.round(hours / 24), "day");
}

function DevicesCard({ user }: { user: ClerkUser }) {
  const { session } = useSession();
  const [sessions, setSessions] = React.useState<DeviceSession[] | null>(null);
  const [busy, setBusy] = React.useState<string | null>(null);

  React.useEffect(() => {
    let live = true;
    user
      .getSessions()
      .then((result) => live && setSessions(result))
      .catch(() => live && setSessions([]));
    return () => {
      live = false;
    };
  }, [user]);

  async function revoke(target: DeviceSession) {
    setBusy(target.id);
    try {
      await target.revoke();
      setSessions(await user.getSessions());
      toast.success("Device signed out.");
    } catch (err) {
      toast.error(clerkError(err, "Could not sign out that device."));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Section
      title="Active devices"
      description="Everywhere you're currently signed in."
    >
      {sessions === null ? (
        <div className="grid gap-2">
          <Skeleton className="h-14 rounded-lg" />
          <Skeleton className="h-14 rounded-lg" />
        </div>
      ) : sessions.length === 0 ? (
        <EmptyRow>No other active devices.</EmptyRow>
      ) : (
        <ul className="grid gap-2">
          {sessions.map((item) => {
            const isCurrent = item.id === session?.id;
            const pending = busy === item.id;
            return (
              <li
                key={item.id}
                className="flex items-center gap-3 rounded-lg border border-border/60 px-3 py-2.5"
              >
                <HugeiconsIcon
                  icon={
                    item.latestActivity.isMobile
                      ? SmartPhone01Icon
                      : ComputerIcon
                  }
                  strokeWidth={2}
                  className="size-4 shrink-0 text-muted-foreground"
                />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm">{deviceLabel(item)}</p>
                  <p className="truncate text-xs text-muted-foreground">
                    {deviceLocation(item)} · {lastActive(item.lastActiveAt)}
                  </p>
                </div>
                {isCurrent ? (
                  <Badge variant="secondary">This device</Badge>
                ) : pending ? (
                  <Spinner className="mx-2 text-muted-foreground" />
                ) : (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => revoke(item)}
                    className="text-muted-foreground hover:text-destructive"
                  >
                    Sign out
                  </Button>
                )}
              </li>
            );
          })}
        </ul>
      )}
    </Section>
  );
}
