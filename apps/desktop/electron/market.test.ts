import assert from "node:assert/strict";
import test from "node:test";
import {
  buildMarketWatchlist,
  formatMarketMenuLabel,
  formatMarketTrayTitle,
  marketSessionLabel,
  parseMarketPriceUpdate,
  type MarketQuote,
} from "./market.ts";

const quote: MarketQuote = {
  symbol: "AAPL",
  name: "Apple Inc.",
  price: 225.5,
  change: 2.5,
  changePercent: 1.12,
  regularMarketPrice: 225.5,
  preMarketPrice: null,
  postMarketPrice: null,
  currency: "USD",
  currencySymbol: "$",
  exchange: "NasdaqGS",
  marketState: "REGULAR",
  marketTime: "2026-08-06T15:00:00Z",
  isStale: false,
};

test("watchlist favors recent traded symbols and removes duplicates", () => {
  assert.deepEqual(buildMarketWatchlist([" tsla ", "AAPL", "tsla"], 5), [
    "TSLA",
    "AAPL",
    "SPY",
    "QQQ",
    "DIA",
  ]);
});

test("market labels include symbol, price, and direction", () => {
  assert.match(formatMarketMenuLabel(quote), /AAPL\s+\$225\.50\s+▲ 1\.12%/);
  assert.equal(formatMarketTrayTitle(quote), " AAPL $225.50 ▲ 1.12%");
  assert.equal(marketSessionLabel("POST"), "After hours");
});

test("live price updates are validated", () => {
  assert.equal(parseMarketPriceUpdate({
    symbol: "AAPL",
    price: 226,
    change: 3,
    changePercent: 1.35,
    currency: "USD",
    exchange: "NMS",
    marketState: "REGULAR",
    marketTime: "2026-08-06T17:00:00Z",
  }).price, 226);
  assert.throws(() => parseMarketPriceUpdate({ symbol: "AAPL" }), /Invalid market price update/);
});
