import { contextBridge, ipcRenderer } from "electron";
import type { DesktopBridge, DesktopEvent } from "../src/ipc/contract.ts";

const bridge: DesktopBridge = {
  invoke: (command, args) => ipcRenderer.invoke("tradstry:invoke", command, args),
  listen: (event, listener) => {
    const handler = (_electronEvent: Electron.IpcRendererEvent, message: DesktopEvent) => {
      if (message.event === event) listener(message as DesktopEvent<never>);
    };
    ipcRenderer.on("tradstry:event", handler);
    return () => ipcRenderer.removeListener("tradstry:event", handler);
  },
  mediaUrl: (path) => `tradstry-media://local${encodeURI(path)}`,
  openExternal: (url) => ipcRenderer.invoke("tradstry:open-external", url),
  setTheme: (theme) => ipcRenderer.invoke("tradstry:set-theme", theme),
  subscribe: (query, variables, handlers) => {
    const id = crypto.randomUUID();
    const handler = (
      _event: Electron.IpcRendererEvent,
      message: { id: string; type: "data" | "error" | "complete"; data?: unknown; message?: string },
    ) => {
      if (message.id !== id) return;
      if (message.type === "data") handlers.onMessage(message.data as never);
      if (message.type === "error") handlers.onError?.(new Error(message.message ?? "Subscription failed"));
      if (message.type === "complete") handlers.onComplete?.();
    };
    ipcRenderer.on("tradstry:graphql-event", handler);
    ipcRenderer.send("tradstry:graphql-subscribe", { id, query, variables });
    return () => {
      ipcRenderer.removeListener("tradstry:graphql-event", handler);
      ipcRenderer.send("tradstry:graphql-unsubscribe", id);
    };
  },
};

contextBridge.exposeInMainWorld("tradstry", bridge);
