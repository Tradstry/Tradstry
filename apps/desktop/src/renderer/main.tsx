import React from "react";
import ReactDOM from "react-dom/client";
import "../index.css";
import App from "../App";
import { TooltipProvider } from "@tradstry/app-ui/components/ui/tooltip";
import { Toaster } from "@tradstry/app-ui/components/ui/sonner";

document.documentElement.classList.add("desktop-shell");

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <TooltipProvider delayDuration={400}>
      <App />
      <Toaster position="top-right" />
    </TooltipProvider>
  </React.StrictMode>,
);
