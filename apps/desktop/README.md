# Tradstry Desktop

Electron desktop application with a React renderer and a sandboxed preload bridge. Electron lifecycle, IPC, authentication, offline SQLite, sync, and media operations all live inside this application. Only code shared with the website belongs under `packages/`.

```bash
bun run dev
bun run typecheck
bun run test:main
bun run build
```

The renderer has no Node access. Add desktop capabilities under `electron/` and expose only typed calls using `src/ipc/contract.ts` through the sandboxed preload.
