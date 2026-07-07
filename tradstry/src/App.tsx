import { useEffect, useState } from "react";
import {
  GlassMaterialVariant,
  setLiquidGlassEffect,
} from "tauri-plugin-liquid-glass-api";
import { Header } from "./components/user-interface";
import ZanedLayout from "./components/zaned/layout";
import JournalLayout from "./components/journal/layout";
import SignInScreen from "./components/auth/sign-in-screen";
import { getAuthStatus, signOut, type AuthStatus } from "./auth";

function App() {
  const [mode, setMode] = useState("zaned");
  // null = still checking the keychain on startup.
  const [auth, setAuth] = useState<AuthStatus | null>(null);

  useEffect(() => {
    setLiquidGlassEffect({
      cornerRadius: 12,
      variant: GlassMaterialVariant.Regular,
    }).catch(() => {
      // Older macOS falls back to vibrancy natively; other platforms no-op.
    });
  }, []);

  useEffect(() => {
    getAuthStatus()
      .then(setAuth)
      .catch(() => setAuth({ signedIn: false }));
  }, []);

  const handleSignOut = async () => {
    try {
      await signOut();
    } finally {
      setAuth({ signedIn: false });
    }
  };

  if (auth === null) {
    return <div className="h-screen bg-zinc-50 dark:bg-zinc-950" />;
  }

  if (!auth.signedIn) {
    return <SignInScreen onSignedIn={setAuth} />;
  }

  return (
    <div className="flex h-screen flex-col">
      <Header
        mode={mode}
        onModeChange={setMode}
        userName={auth.name}
        userEmail={auth.email}
        onSignOut={handleSignOut}
      />
      {mode === "zaned" ? <ZanedLayout /> : <JournalLayout />}
    </div>
  );
}

export default App;
