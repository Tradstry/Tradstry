import { app, BrowserWindow, ipcMain, protocol } from "electron";
import {
  createGraphqlClient,
  DesktopService,
  MediaRepository,
  MediaSync,
  openDesktopDatabase,
  RemoteAnalytics,
  startBackgroundSync,
  SyncEngine,
  SyncProtocol,
} from "./sync/index.ts";
import { readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { extname, join, resolve, sep } from "node:path";
import { DesktopAuth } from "./auth";

let window: BrowserWindow | null = null;
let service: DesktopService | null = null;
let stopSync: (() => void) | null = null;

function schemaPath(): string {
  if (app.isPackaged) return join(process.resourcesPath, "schema.sql");
  return join(app.getAppPath(), "electron/sync/schema.sql");
}

function createService(): DesktopService {
  const dataDirectory = app.getPath("userData");
  const store = openDesktopDatabase(join(dataDirectory, "notebook.db"), readFileSync(schemaPath(), "utf8"));
  const auth = new DesktopAuth(dataDirectory);
  const backendUrl = process.env.TRADSTRY_BACKEND_URL ?? "http://localhost:7899/graphql";
  const graphql = createGraphqlClient({ endpoint: backendUrl, getAccessToken: () => auth.accessToken() });
  const protocolClient = new SyncProtocol(graphql);
  const mediaSync = new MediaSync({
    db: store.db,
    backendUrl,
    getAccessToken: () => auth.accessToken(),
    onProgress: (payload) => window?.webContents.send("tradstry:event", { event: "notebook://media-progress", payload }),
  });
  const engine = new SyncEngine(store, protocolClient, { media: mediaSync });
  const media = new MediaRepository({
    db: store.db,
    media: mediaSync,
    dataDirectory,
    downloadsDirectory: app.getPath("downloads"),
  });
  stopSync = startBackgroundSync(engine, {
    shouldSync: async () => (await auth.status()).signedIn,
    onUpdates: (payload) => window?.webContents.send("tradstry:event", { event: "notebook://updates", payload }),
  });
  return new DesktopService({ store, auth, sync: engine, graphql, analytics: new RemoteAnalytics(graphql), media });
}

async function createWindow() {
  window = new BrowserWindow({
    width: 1440,
    height: 920,
    minWidth: 1000,
    minHeight: 700,
    titleBarStyle: process.platform === "darwin" ? "hiddenInset" : "default",
    vibrancy: process.platform === "darwin" ? "under-window" : undefined,
    visualEffectState: process.platform === "darwin" ? "active" : undefined,
    backgroundColor: "#00000000",
    webPreferences: {
      preload: join(__dirname, "../preload/preload.cjs"),
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  });

  if (process.env.ELECTRON_RENDERER_URL) {
    await window.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    await window.loadFile(join(__dirname, "../renderer/index.html"));
  }
  window.on("focus", () => void service?.invoke("sync_now"));
}

protocol.registerSchemesAsPrivileged([
  { scheme: "tradstry-media", privileges: { secure: true, standard: true, supportFetchAPI: true } },
]);

app.whenReady().then(async () => {
  protocol.handle("tradstry-media", async (request) => {
    const url = new URL(request.url);
    const mediaRoot = resolve(app.getPath("userData"), "media");
    const filePath = resolve(decodeURIComponent(url.pathname));
    if (!filePath.startsWith(`${mediaRoot}${sep}`)) {
      return new Response("Forbidden", { status: 403 });
    }
    try {
      const body = await readFile(filePath);
      const contentType = extname(filePath).toLowerCase() === ".jpg" ? "image/jpeg" : undefined;
      return new Response(body, { headers: contentType ? { "content-type": contentType } : {} });
    } catch {
      return new Response("Not found", { status: 404 });
    }
  });

  service = createService();
  ipcMain.handle("tradstry:invoke", (_event, command: string, args?: Record<string, unknown>) => {
    if (!service) throw new Error("Desktop service is not ready");
    return service.invoke(command, args ?? {});
  });

  await createWindow();
  app.on("activate", () => {
    if (BrowserWindow.getAllWindows().length === 0) void createWindow();
  });
});

app.on("before-quit", () => {
  stopSync?.();
  service?.close();
});
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
