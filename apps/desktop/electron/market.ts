export type MarketQuote = {
  symbol: string;
  name: string;
  price: number | null;
  change: number | null;
  changePercent: number | null;
  regularMarketPrice: number | null;
  preMarketPrice: number | null;
  postMarketPrice: number | null;
  currency: string | null;
  currencySymbol: string | null;
  exchange: string | null;
  marketState: string;
  marketTime: string | null;
  isStale: boolean;
};

export type MarketQuotesPayload = {
  quotes: MarketQuote[];
  errors: Array<{ symbol: string; message: string }>;
  fetchedAt: string;
};

export type MarketPriceUpdate = {
  symbol: string;
  price: number;
  change: number;
  changePercent: number;
  currency: string;
  exchange: string;
  marketState: string;
  marketTime: string;
};

const DEFAULT_SYMBOLS = ["SPY", "QQQ", "DIA", "AAPL", "NVDA"];

export function buildMarketWatchlist(recentSymbols: unknown, limit = 8): string[] {
  const recent = Array.isArray(recentSymbols)
    ? recentSymbols.filter((symbol): symbol is string => typeof symbol === "string")
    : [];
  const seen = new Set<string>();
  const symbols: string[] = [];
  for (const value of [...recent, ...DEFAULT_SYMBOLS]) {
    const symbol = value.trim().toUpperCase();
    if (!symbol || seen.has(symbol)) continue;
    seen.add(symbol);
    symbols.push(symbol);
    if (symbols.length === limit) break;
  }
  return symbols;
}

export function parseMarketQuotesPayload(value: unknown): MarketQuotesPayload {
  if (!value || typeof value !== "object") throw new Error("Invalid market quote response");
  const payload = value as Partial<MarketQuotesPayload>;
  if (!Array.isArray(payload.quotes) || !Array.isArray(payload.errors) || typeof payload.fetchedAt !== "string") {
    throw new Error("Invalid market quote response");
  }
  return payload as MarketQuotesPayload;
}

export function parseMarketPriceUpdate(value: unknown): MarketPriceUpdate {
  if (!value || typeof value !== "object") throw new Error("Invalid market price update");
  const update = value as Partial<MarketPriceUpdate>;
  if (
    typeof update.symbol !== "string"
    || typeof update.price !== "number"
    || typeof update.change !== "number"
    || typeof update.changePercent !== "number"
    || typeof update.currency !== "string"
    || typeof update.exchange !== "string"
    || typeof update.marketState !== "string"
    || typeof update.marketTime !== "string"
  ) {
    throw new Error("Invalid market price update");
  }
  return update as MarketPriceUpdate;
}

function priceFormatter(currency: string | null): Intl.NumberFormat {
  try {
    return new Intl.NumberFormat("en-US", {
      style: "currency",
      currency: currency || "USD",
      minimumFractionDigits: 2,
      maximumFractionDigits: 2,
    });
  } catch {
    return new Intl.NumberFormat("en-US", { minimumFractionDigits: 2, maximumFractionDigits: 2 });
  }
}

export function formatMarketPrice(quote: MarketQuote): string {
  if (quote.price === null || !Number.isFinite(quote.price)) return "—";
  return priceFormatter(quote.currency).format(quote.price);
}

export function formatMarketChange(quote: MarketQuote): string {
  if (quote.changePercent === null || !Number.isFinite(quote.changePercent)) return "—";
  const direction = quote.changePercent >= 0 ? "▲" : "▼";
  return `${direction} ${Math.abs(quote.changePercent).toFixed(2)}%`;
}

export function formatMarketMenuLabel(quote: MarketQuote): string {
  const stale = quote.isStale ? "◌ " : "";
  return `${stale}${quote.symbol}   ${formatMarketPrice(quote)}   ${formatMarketChange(quote)}`;
}

export function formatMarketTrayTitle(quote: MarketQuote): string {
  return ` ${quote.symbol} ${formatMarketPrice(quote)} ${formatMarketChange(quote)}`;
}

export function marketSessionLabel(state: string): string {
  switch (state.toUpperCase()) {
    case "PRE":
    case "PREPRE":
      return "Pre-market";
    case "REGULAR":
      return "Market open";
    case "POST":
    case "POSTPOST":
      return "After hours";
    case "CLOSED":
      return "Market closed";
    default:
      return "Market data";
  }
}
