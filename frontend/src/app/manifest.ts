import type { MetadataRoute } from "next";

export default function manifest(): MetadataRoute.Manifest {
  return {
    name: "Tradstry",
    short_name: "Tradstry",
    description: "Trading journal and analytics",
    start_url: "/",
    display: "standalone",
    background_color: "#ffffff",
    theme_color: "#101012",
    icons: [
      {
        src: "/icon-192.png",
        sizes: "192x192",
        type: "image/png",
        purpose: "any",
      },
      {
        src: "/icon-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "any",
      },
      {
        // Android crops a maskable icon to a circle, so this one carries the mark inside
        // the safe zone. Pointing `maskable` at the rounded icon (as it did) means the
        // corners get sliced off.
        src: "/icon-maskable-512.png",
        sizes: "512x512",
        type: "image/png",
        purpose: "maskable",
      },
    ],
  };
}
