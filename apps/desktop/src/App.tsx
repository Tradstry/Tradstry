import {
  DashboardApp,
  TradstryProvider,
  type GraphQLFetcher,
  type GraphQLSubscriber,
  type TradstryPlatform,
  type TradstryTheme,
} from "@tradstry/app-ui";
import { useEffect, useMemo, useState } from "react";
import SignInScreen from "./components/auth/sign-in-screen";
import { getAuthStatus, signOut, type AuthStatus } from "./auth";

const fetcher: GraphQLFetcher = (query, variables) =>
  window.tradstry.invoke("graphql_query", { query, variables });

const subscriber: GraphQLSubscriber = (query, variables, handlers) =>
  window.tradstry.subscribe(query, variables, handlers);

const backendBaseUrl = import.meta.env.VITE_TRADSTRY_BACKEND_BASE_URL
  || (import.meta.env.DEV ? "http://localhost:7899" : "https://backend.tradstry.com");

function applyTheme(theme: TradstryTheme): void {
  const dark = theme === "dark" || (
    theme === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches
  );
  document.documentElement.classList.toggle("dark", dark);
}

function DesktopDashboard({ auth, onSignOut }: {
  auth: AuthStatus & { signedIn: true };
  onSignOut: () => Promise<void>;
}) {
  const [pathname, setPathname] = useState("/dashboard");
  const [theme, setThemeState] = useState<TradstryTheme>("system");

  useEffect(() => applyTheme(theme), [theme]);

  const platform = useMemo<TradstryPlatform>(() => ({
    auth: {
      isLoaded: true,
      isSignedIn: true,
      getToken: () => window.tradstry.invoke<string | null>("auth_token"),
    },
    user: {
      fullName: auth.name ?? "User",
      email: auth.email ?? "",
    },
    pathname,
    appBaseUrl: "https://tradstry.com",
    backendBaseUrl,
    navigate: setPathname,
    openExternal: window.tradstry.openExternal,
    signOut: onSignOut,
    theme,
    setTheme: setThemeState,
  }), [auth.email, auth.name, onSignOut, pathname, theme]);

  return (
    <TradstryProvider platform={platform} fetcher={fetcher} subscriber={subscriber}>
      <DashboardApp pathname={pathname} />
    </TradstryProvider>
  );
}

export default function App() {
  const [auth, setAuth] = useState<AuthStatus | null>(null);

  useEffect(() => {
    getAuthStatus().then(setAuth).catch(() => setAuth({ signedIn: false }));
  }, []);

  const handleSignOut = async () => {
    try {
      await signOut();
    } finally {
      setAuth({ signedIn: false });
    }
  };

  if (auth === null) {
    return <div className="h-screen bg-background" />;
  }
  if (!auth.signedIn) {
    return <SignInScreen onSignedIn={setAuth} />;
  }
  return <DesktopDashboard auth={auth as AuthStatus & { signedIn: true }} onSignOut={handleSignOut} />;
}
