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
};

contextBridge.exposeInMainWorld("tradstry", bridge);
