import React from "react";
import ReactDOM from "react-dom/client";
import "../index.css";
import App from "../App";
import { TooltipProvider } from "@tradstry/app-ui/components/ui/tooltip";
import { Toaster } from "@tradstry/app-ui/components/ui/sonner";

// shadcn theming is class-based (`.dark` on <html>). Mirror the OS appearance
// onto that class so dark mode follows macOS and the theme tokens switch.
const darkQuery = window.matchMedia("(prefers-color-scheme: dark)");
document.documentElement.classList.add("desktop-shell");
const syncTheme = (dark: boolean) =>
  document.documentElement.classList.toggle("dark", dark);
syncTheme(darkQuery.matches);
darkQuery.addEventListener("change", (event) => syncTheme(event.matches));

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TooltipProvider delayDuration={400}>
      <App />
      <Toaster position="top-right" />
    </TooltipProvider>
  </React.StrictMode>,
);
