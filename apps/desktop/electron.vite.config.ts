import { resolve } from "node:path";
import { defineConfig, externalizeDepsPlugin } from "electron-vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import { loadEnv } from "vite";

const PRODUCTION_CLERK_PUBLISHABLE_KEY = "pk_live_Y2xlcmsudHJhZHN0cnkuY29tJA";
const PRODUCTION_CLERK_OAUTH_CLIENT_ID = "uwCxAuVrIvhYzK1v";
const LOCAL_BACKEND_URL = "http://localhost:7899/graphql";
const PRODUCTION_BACKEND_URL = "https://backend.tradstry.com/graphql";

export default defineConfig(({ mode }) => {
  const env = loadEnv(mode, resolve("."), "");
  const backendUrl = env.TRADSTRY_BACKEND_URL
    || (mode === "development" ? LOCAL_BACKEND_URL : PRODUCTION_BACKEND_URL);
  const scriptSource = mode === "development"
    ? "script-src 'self' 'unsafe-inline'"
    : "script-src 'self'";

  return {
    main: {
      define: {
        "process.env.CLERK_OAUTH_CLIENT_ID": JSON.stringify(
          env.CLERK_OAUTH_CLIENT_ID || PRODUCTION_CLERK_OAUTH_CLIENT_ID,
        ),
        "process.env.VITE_CLERK_PUBLISHABLE_KEY": JSON.stringify(
          env.VITE_CLERK_PUBLISHABLE_KEY || PRODUCTION_CLERK_PUBLISHABLE_KEY,
        ),
        "process.env.TRADSTRY_BACKEND_URL": JSON.stringify(backendUrl),
      },
      plugins: [externalizeDepsPlugin()],
      build: {
        rollupOptions: {
          input: resolve("electron/main.ts"),
          output: { entryFileNames: "main.js" },
        },
      },
    },
    preload: {
      plugins: [externalizeDepsPlugin()],
      build: {
        rollupOptions: {
          input: resolve("electron/preload.ts"),
          output: { format: "cjs", entryFileNames: "preload.cjs" },
        },
      },
    },
    renderer: {
      resolve: {
        alias: {
          "@": resolve("src"),
          "@tradstry/app-ui": resolve("../../packages/app-ui/src"),
        },
      },
      plugins: [
        {
          name: "tradstry-renderer-csp",
          transformIndexHtml: (html) => html.replace("__TRADSTRY_SCRIPT_SRC__", scriptSource),
        },
        react(),
        tailwindcss(),
      ],
    },
  };
});
