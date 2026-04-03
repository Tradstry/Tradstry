import type { NextConfig } from "next";
import path from "path";

const nextConfig: NextConfig = {
  /* config options here */
  reactCompiler: true,
  turbopack: {
    resolveAlias: {
      tailwindcss: path.resolve(__dirname, "node_modules/tailwindcss"),
      "tw-animate-css": path.resolve(
        __dirname,
        "node_modules/tw-animate-css"
      ),
      "shadcn/tailwind.css": path.resolve(
        __dirname,
        "node_modules/shadcn/tailwind.css"
      ),
    },
  },
};

export default nextConfig;
