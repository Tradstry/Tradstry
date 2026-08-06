# Tradstry Desktop

Electron desktop application with a React renderer and a sandboxed preload bridge. Electron lifecycle, IPC, authentication, offline SQLite, sync, and media operations all live inside this application. Only code shared with the website belongs under `packages/`.

```bash
bun run dev
bun run typecheck
bun run test:main
bun run build
```

Desktop sign-in uses Clerk's public OAuth client with Authorization Code +
PKCE. Development identifiers are documented in `.env.example` and must match
the Clerk instance configured by `backend/.env`. Production builds fall back
to the live public identifiers in `electron.vite.config.ts`. The registered
redirect URI is `http://127.0.0.1:8788/callback`.

The renderer has no Node access. Add desktop capabilities under `electron/` and expose only typed calls using `src/ipc/contract.ts` through the sandboxed preload.
