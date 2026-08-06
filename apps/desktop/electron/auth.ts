import { createHash, randomBytes } from "node:crypto";
import { existsSync, readFileSync, unlinkSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { join } from "node:path";
import { safeStorage, shell } from "electron";
import type { AuthCommands } from "./sync/index.ts";

const REDIRECT_PORT = 8788;
const REDIRECT_URI = `http://127.0.0.1:${REDIRECT_PORT}/callback`;
const CLIENT_ID = requiredConfig("CLERK_OAUTH_CLIENT_ID", process.env.CLERK_OAUTH_CLIENT_ID);
const PUBLISHABLE_KEY = requiredConfig("VITE_CLERK_PUBLISHABLE_KEY", process.env.VITE_CLERK_PUBLISHABLE_KEY);
const SUCCESS_HTML = `<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Signed in · Tradstry</title><style>:root{color-scheme:light dark}body{min-height:100vh;margin:0;display:grid;place-items:center;font:14px system-ui;background:#fafafa;color:#18181b}.card{text-align:center}h1{font-size:20px}@media(prefers-color-scheme:dark){body{background:#09090b;color:#fafafa}}</style><main class="card"><h1>Signed in to Tradstry</h1><p>You can close this tab and return to the app.</p></main>`;

type StoredTokens = {
  accessToken: string;
  refreshToken: string | null;
  expiresAt: number;
  email: string | null;
  name: string | null;
};

type TokenResponse = {
  access_token: string;
  refresh_token?: string;
  expires_in?: number;
};

export type AuthStatus = { signedIn: boolean; email: string | null; name: string | null };

export class DesktopAuth implements AuthCommands {
  readonly #path: string;
  readonly #baseUrl: string;
  #refreshing: Promise<AuthStatus> | null = null;

  constructor(dataDirectory: string) {
    this.#path = join(dataDirectory, "auth.tokens");
    this.#baseUrl = clerkBaseUrl(PUBLISHABLE_KEY);
  }

  async signIn(): Promise<AuthStatus> {
    if (!safeStorage.isEncryptionAvailable()) throw new Error("OS credential encryption is unavailable");
    const verifier = randomBytes(32).toString("base64url");
    const challenge = createHash("sha256").update(verifier).digest("base64url");
    const state = randomBytes(24).toString("base64url");
    const authorization = new URL(`${this.#baseUrl}/oauth/authorize`);
    authorization.search = new URLSearchParams({
      client_id: CLIENT_ID,
      response_type: "code",
      redirect_uri: REDIRECT_URI,
      scope: "openid profile email offline_access",
      state,
      code_challenge: challenge,
      code_challenge_method: "S256",
    }).toString();

    const callback = waitForCallback();
    await shell.openExternal(authorization.toString());
    const returned = await callback;
    if (returned.searchParams.get("state") !== state) throw new Error("State mismatch — possible CSRF, aborting");
    const code = returned.searchParams.get("code");
    if (!code) throw new Error(returned.searchParams.get("error_description") ?? "No authorization code returned");

    const token = await this.#token({
      grant_type: "authorization_code",
      code,
      redirect_uri: REDIRECT_URI,
      client_id: CLIENT_ID,
      code_verifier: verifier,
    });
    const profile = await this.#userinfo(token.access_token);
    this.#save({
      accessToken: token.access_token,
      refreshToken: token.refresh_token ?? null,
      expiresAt: unixNow() + (token.expires_in ?? 3600),
      email: profile.email,
      name: profile.name,
    });
    return { signedIn: true, email: profile.email, name: profile.name };
  }

  async status(): Promise<AuthStatus> {
    const tokens = this.#load();
    if (!tokens) return signedOut();
    if (tokens.expiresAt > unixNow() + 60) return statusFrom(tokens);
    if (!tokens.refreshToken) {
      this.#clear();
      return signedOut();
    }
    try {
      return await this.#refresh(tokens);
    } catch {
      this.#clear();
      return signedOut();
    }
  }

  async signOut(): Promise<void> {
    this.#clear();
  }

  async accessToken(): Promise<string | null> {
    const tokens = this.#load();
    if (!tokens) return null;
    if (tokens.expiresAt > unixNow() + 60) return tokens.accessToken;
    if (!tokens.refreshToken) return null;
    try {
      await this.#refresh(tokens);
      return this.#load()?.accessToken ?? null;
    } catch {
      this.#clear();
      return null;
    }
  }

  async #refresh(previous: StoredTokens): Promise<AuthStatus> {
    if (this.#refreshing) return this.#refreshing;
    this.#refreshing = (async () => {
      const token = await this.#token({
        grant_type: "refresh_token",
        refresh_token: previous.refreshToken!,
        client_id: CLIENT_ID,
      });
      const next: StoredTokens = {
        accessToken: token.access_token,
        refreshToken: token.refresh_token ?? previous.refreshToken,
        expiresAt: unixNow() + (token.expires_in ?? 3600),
        email: previous.email,
        name: previous.name,
      };
      this.#save(next);
      return statusFrom(next);
    })();
    try {
      return await this.#refreshing;
    } finally {
      this.#refreshing = null;
    }
  }

  async #token(parameters: Record<string, string>): Promise<TokenResponse> {
    const response = await fetch(`${this.#baseUrl}/oauth/token`, {
      method: "POST",
      redirect: "manual",
      headers: { "content-type": "application/x-www-form-urlencoded" },
      body: new URLSearchParams(parameters),
    });
    const payload = (await response.json()) as TokenResponse & { error_description?: string };
    if (!response.ok || !payload.access_token) throw new Error(payload.error_description ?? `OAuth token request failed (${response.status})`);
    return payload;
  }

  async #userinfo(accessToken: string): Promise<{ email: string | null; name: string | null }> {
    try {
      const response = await fetch(`${this.#baseUrl}/oauth/userinfo`, {
        headers: { authorization: `Bearer ${accessToken}` },
        redirect: "manual",
      });
      const value = (await response.json()) as Record<string, unknown>;
      const email = typeof value.email === "string" ? value.email : null;
      const explicit = typeof value.name === "string" && value.name ? value.name : null;
      const combined = [value.given_name, value.family_name].filter((item): item is string => typeof item === "string").join(" ").trim();
      return { email, name: explicit ?? (combined || null) };
    } catch {
      return { email: null, name: null };
    }
  }

  #save(tokens: StoredTokens): void {
    writeFileSync(this.#path, safeStorage.encryptString(JSON.stringify(tokens)), { mode: 0o600 });
  }

  #load(): StoredTokens | null {
    if (!existsSync(this.#path) || !safeStorage.isEncryptionAvailable()) return null;
    try {
      return JSON.parse(safeStorage.decryptString(readFileSync(this.#path))) as StoredTokens;
    } catch {
      return null;
    }
  }

  #clear(): void {
    try {
      unlinkSync(this.#path);
    } catch {}
  }
}

function clerkBaseUrl(key: string): string {
  const payload = key.replace(/^pk_(?:test|live)_/, "").replace(/=+$/, "");
  const host = Buffer.from(payload, "base64").toString("utf8").replace(/\$$/, "");
  if (!host) throw new Error("Invalid Clerk publishable key");
  return `https://${host}`;
}

function requiredConfig(name: string, value: string | undefined): string {
  const normalized = value?.trim();
  if (!normalized) throw new Error(`${name} is required to build the desktop app`);
  return normalized;
}

function waitForCallback(): Promise<URL> {
  return new Promise((resolve, reject) => {
    const timer = setTimeout(() => {
      server.close();
      reject(new Error("Sign-in timed out"));
    }, 300_000);
    const server = createServer((request, response) => {
      clearTimeout(timer);
      response.writeHead(200, { "content-type": "text/html; charset=utf-8", "content-length": Buffer.byteLength(SUCCESS_HTML) });
      response.end(SUCCESS_HTML);
      server.close();
      try {
        resolve(new URL(request.url ?? "/", REDIRECT_URI));
      } catch (error) {
        reject(error);
      }
    });
    server.once("error", (error) => {
      clearTimeout(timer);
      reject(new Error(`Cannot start OAuth callback listener: ${error.message}`));
    });
    server.listen(REDIRECT_PORT, "127.0.0.1");
  });
}

function unixNow(): number {
  return Math.trunc(Date.now() / 1000);
}
function signedOut(): AuthStatus {
  return { signedIn: false, email: null, name: null };
}
function statusFrom(tokens: StoredTokens): AuthStatus {
  return { signedIn: true, email: tokens.email, name: tokens.name };
}
