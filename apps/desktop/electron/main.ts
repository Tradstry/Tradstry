import {
  app,
  BrowserWindow,
  ipcMain,
  Menu,
  nativeImage,
  protocol,
  shell,
  Tray,
  type MenuItemConstructorOptions,
} from "electron";
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
import { GraphqlSubscriptions } from "./graphql-subscriptions";
import {
  buildMarketWatchlist,
  formatMarketMenuLabel,
  formatMarketTrayTitle,
  marketSessionLabel,
  parseMarketPriceUpdate,
  parseMarketQuotesPayload,
  type MarketQuote,
} from "./market";

let window: BrowserWindow | null = null;
let service: DesktopService | null = null;
let stopSync: (() => void) | null = null;
let subscriptions: GraphqlSubscriptions | null = null;
let tray: Tray | null = null;
let marketQuotes: MarketQuote[] = [];
let marketError: string | null = null;
let marketFetchedAt: string | null = null;
let marketRefreshTimer: ReturnType<typeof setInterval> | null = null;
let marketRotationTimer: ReturnType<typeof setInterval> | null = null;
let marketRotationIndex = 0;
let marketRefreshRunning = false;
let marketSubscriptionKey: string | null = null;
let failedMarketSubscriptionKey: string | null = null;

const MARKET_REFRESH_INTERVAL_MS = 30_000;
const MARKET_ROTATION_INTERVAL_MS = 6_000;
const MARKET_SUBSCRIPTION_ID = "desktop-market-ticker";
const MARKET_PRICE_SUBSCRIPTION = `subscription DesktopMarketPriceUpdates($symbols: [String!]!) {
  marketPriceUpdates(symbols: $symbols) {
    symbol price change changePercent currency exchange marketState marketTime
  }
}`;

app.setName("Tradstry");

async function syncNow(): Promise<void> {
  if (!service) return;
  try {
    await service.invoke("sync_now");
  } catch (error) {
    console.error("Manual sync failed:", error);
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function updateTrayTitle(): void {
  if (!tray || process.platform !== "darwin") return;
  if (marketQuotes.length === 0) {
    tray.setTitle("");
    return;
  }
  const quote = marketQuotes[marketRotationIndex % marketQuotes.length];
  if (quote) tray.setTitle(formatMarketTrayTitle(quote));
}

function updateTrayMenu(): void {
  if (!tray) return;
  const menu: MenuItemConstructorOptions[] = [];
  if (marketQuotes.length > 0) {
    const states = new Set(marketQuotes.map((quote) => marketSessionLabel(quote.marketState)));
    menu.push({ label: states.size === 1 ? [...states][0] : "Live market prices", enabled: false });
    for (const quote of marketQuotes) {
      menu.push({
        label: formatMarketMenuLabel(quote),
        toolTip: `${quote.name}${quote.exchange ? ` · ${quote.exchange}` : ""}`,
        click: () => void shell.openExternal(`https://finance.yahoo.com/quote/${encodeURIComponent(quote.symbol)}`),
      });
    }
  } else {
    menu.push({ label: marketError ? "Prices unavailable" : "Loading live prices…", enabled: false });
  }
  if (marketError) menu.push({ label: marketError, enabled: false });
  if (marketFetchedAt) {
    const updatedAt = new Date(marketFetchedAt);
    if (!Number.isNaN(updatedAt.getTime())) {
      menu.push({ label: `Updated ${updatedAt.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })}`, enabled: false });
    }
  }
  menu.push(
    { label: marketRefreshRunning ? "Refreshing Prices…" : "Refresh Prices", enabled: !marketRefreshRunning, click: () => void refreshMarketQuotes({ retryLive: true }) },
    { type: "separator" },
    { label: "Open Tradstry", click: showWindow },
    { label: "Sync Now", click: () => void syncNow() },
    { type: "separator" },
    { label: "Quit Tradstry", click: () => app.quit() },
  );
  tray.setContextMenu(Menu.buildFromTemplate(menu));
}

function handleMarketSubscriptionMessage(message:
  | { type: "data"; data: unknown }
  | { type: "error"; message: string }
  | { type: "complete" }): void {
  if (message.type === "error") {
    if (marketSubscriptionKey) {
      failedMarketSubscriptionKey = marketSubscriptionKey;
    }
    console.warn(`Live market stream unavailable; using snapshot prices: ${message.message}`);
    marketSubscriptionKey = null;
    return;
  }
  if (message.type === "complete") {
    marketSubscriptionKey = null;
    return;
  }
  try {
    if (!message.data || typeof message.data !== "object") {
      throw new Error("Live market subscription returned no data");
    }
    const data = message.data as { marketPriceUpdates?: unknown };
    const update = parseMarketPriceUpdate(data.marketPriceUpdates);
    const quote = marketQuotes.find((candidate) => candidate.symbol === update.symbol);
    if (!quote) return;
    quote.price = update.price;
    quote.change = update.change;
    quote.changePercent = update.changePercent;
    quote.currency = update.currency || quote.currency;
    quote.exchange = update.exchange || quote.exchange;
    quote.marketState = update.marketState;
    quote.marketTime = update.marketTime;
    quote.isStale = false;
    marketFetchedAt = update.marketTime;
    failedMarketSubscriptionKey = null;
    updateTrayTitle();
    updateTrayMenu();
  } catch (error) {
    console.error("Invalid live market update:", error);
  }
}

function subscribeToMarketPrices(symbols: string[]): void {
  const key = symbols.join(",");
  if (
    !subscriptions
    || marketSubscriptionKey === key
    || failedMarketSubscriptionKey === key
  ) return;
  subscriptions.unsubscribe(MARKET_SUBSCRIPTION_ID);
  marketSubscriptionKey = key;
  void subscriptions.subscribe(MARKET_SUBSCRIPTION_ID, MARKET_PRICE_SUBSCRIPTION, { symbols });
}

async function refreshMarketQuotes(options: { retryLive?: boolean } = {}): Promise<void> {
  if (!service || marketRefreshRunning) return;
  if (options.retryLive) failedMarketSubscriptionKey = null;
  marketRefreshRunning = true;
  updateTrayMenu();
  try {
    const recentSymbols = await service.invoke("market_watchlist_symbols");
    const symbols = buildMarketWatchlist(recentSymbols);
    const value = await service.invoke("market_quotes", { symbols });
    const payload = parseMarketQuotesPayload(value);
    marketQuotes = payload.quotes;
    marketFetchedAt = payload.fetchedAt;
    marketError = payload.errors.length > 0
      ? payload.errors.map((error) => `${error.symbol}: unavailable`).join(" · ")
      : null;
    marketRotationIndex = 0;
    subscribeToMarketPrices(symbols);
  } catch (error) {
    marketError = errorMessage(error);
  } finally {
    marketRefreshRunning = false;
    updateTrayTitle();
    updateTrayMenu();
  }
}

function startMarketTicker(): void {
  void refreshMarketQuotes();
  marketRefreshTimer = setInterval(() => void refreshMarketQuotes(), MARKET_REFRESH_INTERVAL_MS);
  marketRotationTimer = setInterval(() => {
    if (marketQuotes.length > 0) marketRotationIndex = (marketRotationIndex + 1) % marketQuotes.length;
    updateTrayTitle();
  }, MARKET_ROTATION_INTERVAL_MS);
}

function showWindow(): void {
  if (!window || window.isDestroyed()) {
    void createWindow();
    return;
  }
  if (window.isMinimized()) window.restore();
  window.show();
  window.focus();
}

function createApplicationMenu(): void {
  const template: MenuItemConstructorOptions[] = [];
  const fileMenu: MenuItemConstructorOptions[] = [
    {
      label: "Sync Now",
      accelerator: "CmdOrCtrl+Shift+R",
      click: () => void syncNow(),
    },
    {
      label: "Refresh Market Prices",
      accelerator: "CmdOrCtrl+Shift+P",
      click: () => void refreshMarketQuotes({ retryLive: true }),
    },
    { type: "separator" },
    process.platform === "darwin" ? { role: "close" } : { role: "quit" },
  ];
  const viewMenu: MenuItemConstructorOptions[] = [{ role: "reload" }];
  if (!app.isPackaged) viewMenu.push({ role: "toggleDevTools" });
  viewMenu.push(
    { type: "separator" },
    { role: "resetZoom" },
    { role: "zoomIn" },
    { role: "zoomOut" },
    { type: "separator" },
    { role: "togglefullscreen" },
  );

  if (process.platform === "darwin") {
    template.push({
      label: app.name,
      submenu: [
        { role: "about" },
        { type: "separator" },
        { role: "services" },
        { type: "separator" },
        { role: "hide" },
        { role: "hideOthers" },
        { role: "unhide" },
        { type: "separator" },
        { role: "quit" },
      ],
    });
  }

  template.push(
    {
      label: "File",
      submenu: fileMenu,
    },
    { role: "editMenu" },
    {
      label: "View",
      submenu: viewMenu,
    },
    { role: "windowMenu" },
    {
      role: "help",
      submenu: [
        {
          label: "Tradstry Website",
          click: () => void shell.openExternal("https://tradstry.com"),
        },
      ],
    },
  );

  Menu.setApplicationMenu(Menu.buildFromTemplate(template));
}

function trayIconPath(): string {
  if (app.isPackaged) return join(process.resourcesPath, "tray-icon.png");
  return join(app.getAppPath(), "resources/icons/32x32.png");
}

function createTray(): void {
  const source = nativeImage.createFromPath(trayIconPath());
  if (source.isEmpty()) {
    console.error(`Unable to load tray icon from ${trayIconPath()}`);
    return;
  }

  const icon = process.platform === "darwin" ? source.resize({ width: 16, height: 16 }) : source;
  tray = new Tray(icon);
  tray.setToolTip("Tradstry");
  updateTrayMenu();
  tray.on("click", () => tray?.popUpContextMenu());
}

function schemaPath(): string {
  if (app.isPackaged) return join(process.resourcesPath, "schema.sql");
  return join(app.getAppPath(), "electron/sync/schema.sql");
}

function createService(): DesktopService {
  const dataDirectory = app.getPath("userData");
  const store = openDesktopDatabase(join(dataDirectory, "notebook.db"), readFileSync(schemaPath(), "utf8"));
  const auth = new DesktopAuth(dataDirectory);
  const backendUrl = process.env.TRADSTRY_BACKEND_URL;
  if (!backendUrl) throw new Error("TRADSTRY_BACKEND_URL is required to build the desktop app");
  const graphql = createGraphqlClient({ endpoint: backendUrl, getAccessToken: () => auth.accessToken() });
  subscriptions = new GraphqlSubscriptions({
    endpoint: backendUrl,
    getAccessToken: () => auth.accessToken(),
    emit: (id, message) => {
      if (id === MARKET_SUBSCRIPTION_ID) {
        handleMarketSubscriptionMessage(message);
        return;
      }
      window?.webContents.send("tradstry:graphql-event", { id, ...message });
    },
  });
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

  window.webContents.on("console-message", (details) => {
    if (details.level === "warning" || details.level === "error") {
      console.error(
        `[renderer:${details.level}] ${details.message} (${details.sourceId}:${details.lineNumber})`,
      );
    }
  });
  window.webContents.on(
    "did-fail-load",
    (_event, errorCode, errorDescription, validatedURL, isMainFrame) => {
      if (isMainFrame) {
        console.error(`Renderer failed to load ${validatedURL}: ${errorCode} ${errorDescription}`);
      }
    },
  );
  window.webContents.on("render-process-gone", (_event, details) => {
    console.error(`Renderer process exited: ${details.reason} (${details.exitCode})`);
  });
  if (process.env.ELECTRON_RENDERER_URL) {
    await window.loadURL(process.env.ELECTRON_RENDERER_URL);
  } else {
    await window.loadFile(join(__dirname, "../renderer/index.html"));
  }
  window.on("focus", () => void service?.invoke("sync_now"));
  window.on("closed", () => {
    window = null;
  });
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
  ipcMain.handle("tradstry:open-external", (_event, url: string) => shell.openExternal(url));
  ipcMain.on("tradstry:graphql-subscribe", (_event, request: {
    id: string;
    query: string;
    variables?: Record<string, unknown>;
  }) => {
    void subscriptions?.subscribe(request.id, request.query, request.variables);
  });
  ipcMain.on("tradstry:graphql-unsubscribe", (_event, id: string) => {
    subscriptions?.unsubscribe(id);
  });

  createApplicationMenu();
  createTray();
  startMarketTicker();
  await createWindow();
  app.on("activate", showWindow);
});

app.on("before-quit", () => {
  if (marketRefreshTimer) clearInterval(marketRefreshTimer);
  if (marketRotationTimer) clearInterval(marketRotationTimer);
  marketRefreshTimer = null;
  marketRotationTimer = null;
  tray?.destroy();
  tray = null;
  stopSync?.();
  subscriptions?.close();
  service?.close();
});
app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});
