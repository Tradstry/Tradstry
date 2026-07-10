import type { NextConfig } from "next";
import { createRequire } from "module";
import path from "path";

// Workspace deps are hoisted to the monorepo root, not frontend/node_modules.
const nodeModules = path.dirname(
  path.dirname(createRequire(import.meta.url).resolve("tailwindcss/package.json")),
);

const nextConfig: NextConfig = {
  /* config options here */
  reactCompiler: true,
  // Ships TypeScript source; Next must compile it rather than expect a built dist.
  transpilePackages: ["@tradstry/notebook-core"],
  turbopack: {
    resolveAlias: {
      tailwindcss: path.join(nodeModules, "tailwindcss"),
      "tw-animate-css": path.join(nodeModules, "tw-animate-css"),
      "shadcn/tailwind.css": path.join(nodeModules, "shadcn/tailwind.css"),
    },
  },
};

export default nextConfig;
