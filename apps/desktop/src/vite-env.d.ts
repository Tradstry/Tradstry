/// <reference types="vite/client" />

import type {} from "./ipc/contract";

interface ImportMetaEnv {
  readonly VITE_CLERK_PUBLISHABLE_KEY: string;
  readonly VITE_TRADSTRY_BACKEND_BASE_URL?: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
