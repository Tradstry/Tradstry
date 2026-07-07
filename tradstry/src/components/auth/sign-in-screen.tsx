import { useState } from "react";
import { Button } from "react-aria-components";
import { EnvelopeSimpleIcon, GoogleLogoIcon } from "@phosphor-icons/react";
import { signIn, type AuthStatus } from "../../auth";

type SignInScreenProps = {
  onSignedIn: (status: AuthStatus) => void;
};

const buttonBase =
  "flex h-11 w-full cursor-pointer items-center justify-center gap-2.5 rounded-lg text-sm font-medium outline-none transition duration-150 data-pressed:scale-[0.98] data-disabled:opacity-50 data-focus-visible:outline-2 data-focus-visible:outline-offset-2 data-focus-visible:outline-blue-500 motion-reduce:transition-none";

export default function SignInScreen({ onSignedIn }: SignInScreenProps) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      onSignedIn(await signIn());
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="flex h-screen items-center justify-center bg-zinc-50 dark:bg-zinc-950">
      <div className="flex w-72 flex-col items-center gap-8">
        <div className="flex flex-col items-center gap-1.5">
          <h1 className="text-2xl font-semibold tracking-tight text-zinc-900 dark:text-zinc-50">
            Tradstry
          </h1>
          <p className="text-sm text-zinc-500 dark:text-zinc-400">
            Sign in or create an account
          </p>
        </div>

        <div className="flex w-full flex-col gap-2.5">
          <Button
            onPress={start}
            isDisabled={busy}
            className={`${buttonBase} bg-zinc-900 text-white data-hovered:bg-zinc-800 dark:bg-white dark:text-zinc-900 dark:data-hovered:bg-zinc-200`}
          >
            <GoogleLogoIcon size={18} weight="bold" />
            Continue with Google
          </Button>
          <Button
            onPress={start}
            isDisabled={busy}
            className={`${buttonBase} border border-zinc-300 text-zinc-700 data-hovered:bg-zinc-100 dark:border-zinc-700 dark:text-zinc-200 dark:data-hovered:bg-zinc-900`}
          >
            <EnvelopeSimpleIcon size={18} weight="bold" />
            Continue with email
          </Button>
        </div>

        <p className="h-4 text-center text-xs text-zinc-400 dark:text-zinc-500">
          {busy
            ? "Waiting for your browser…"
            : "Opens your browser to sign in securely"}
        </p>

        {error && (
          <p className="w-full rounded-md bg-red-50 px-3 py-2 text-center text-xs text-red-600 dark:bg-red-950/40 dark:text-red-400">
            {error}
          </p>
        )}
      </div>
    </div>
  );
}
