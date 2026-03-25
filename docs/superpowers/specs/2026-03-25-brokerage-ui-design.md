# Brokerage UI — Design Spec

## Overview

A brokerage page under `/dashboard/brokerage` that displays transaction history synced from SnapTrade via the Go microservice. Data flows: SnapTrade API → Go microservice → Rust backend (persists to Turso) → GraphQL → Frontend.

## Architecture

### Data Flow

```
Page load → auto-sync mutation (syncBrokerageData) → backend decrypts credentials → calls Go microservice → upserts to Turso
         → query (brokerageTransactions) reads from DB → renders table
```

### Three States

1. **Empty** — no brokerage linked on the active account. Centered CTA card: icon, title ("Connect your brokerage"), description, "Connect Account" button, subtitle ("Supports 20+ brokerages via SnapTrade").
2. **Loading** — auto-sync in progress on page load. Skeleton sidebar + skeleton table rows + yellow "Syncing..." indicator in sidebar + thin progress bar above table.
3. **Connected** — sidebar filters + transaction table with pagination.

## Layout

Sidebar-filter layout (matches existing dashboard patterns):

```
┌──────────────────────────────────────────────┐
│ SiteHeader: "Brokerage"                      │
├────────────┬─────────────────────────────────┤
│  Sidebar   │  Main content                   │
│  (200px)   │                                 │
│            │  [progress bar]                 │
│  Sync      │  2,456 transactions   20/page ▾│
│  status    │                                 │
│            │  ┌─────────────────────────────┐│
│  Symbol    │  │ Date  Symbol  Type Qty ...  ││
│  [search]  │  │ Mar22 AAPL   BUY  10  ...  ││
│            │  │ Mar20 MSFT   SELL  5  ...  ││
│  Type      │  │ Mar18 AAPL   DIV  —   ...  ││
│  [badges]  │  │ ...                         ││
│            │  └─────────────────────────────┘│
│  Date      │                                 │
│  [from/to] │  ‹ 1 2 3 ... 123 ›             │
│            │                                 │
│  Desc      │                                 │
│  [search]  │                                 │
│            │                                 │
│  Clear all │                                 │
│  ─────     │                                 │
│  Re-sync   │                                 │
└────────────┴─────────────────────────────────┘
```

## Components

### `BrokeragePage` (`app/dashboard/brokerage/page.tsx`)

Top-level page component. Checks if active account has SnapTrade credentials linked. Renders either `BrokerageEmptyState` or `BrokerageTransactions`.

### `BrokerageEmptyState`

Centered card using the existing `Empty` component pattern. "Connect Account" button triggers the SnapTrade connection flow (calls `linkSnaptradeAccount` mutation after portal redirect).

### `BrokerageTransactions`

Main connected view. Contains:
- `BrokerageFilterSidebar` — left panel with all filter controls
- `BrokerageTable` — right panel with data table + pagination

### `BrokerageFilterSidebar`

Persistent left panel (200px, `border-r`, `bg-muted/30`).

Contents:
- **Sync status** — green "Synced" card with "Last: X min ago" or yellow "Syncing..." during auto-sync
- **Symbol filter** — text input, filters client-side against `symbol.symbol` and `symbol.raw_symbol`
- **Type filter** — toggle badges: BUY (emerald), SELL (rose), DIVIDEND (indigo), TRANSFER (amber), FEE (gray), INTEREST (gray), OTHER (gray). Multiple can be active. All active by default = no filter.
- **Date range** — two date inputs (from/to), filters on `trade_date`
- **Description search** — text input, filters on `description` field
- **Clear all filters** — resets all to default
- **Re-sync button** — manual trigger for `syncBrokerageData`, below a separator

### `BrokerageTable`

Uses `@tanstack/react-table` following the journal-table pattern.

**Columns:**

| Column | Source field | Display |
|--------|-------------|---------|
| Date | `trade_date` | Short date format (Mar 22). Sortable, default descending. |
| Symbol | `symbol_ticker` + `symbol_description` | Ticker bold + description muted, truncated |
| Type | `transaction_type` | Colored badge (BUY=emerald, SELL=rose, DIVIDEND=indigo, TRANSFER=amber, FEE/INTEREST=gray) |
| Qty | `units` | Number, "—" when null/zero (dividends) |
| Price | `price` | Currency formatted |
| Amount | `amount` | Signed currency. Negative = red, positive = green |
| Fee | `fee` | Currency formatted, muted when zero |

**Pagination:** Server-side via GraphQL `offset`/`limit`. Options: 20, 50, 100 per page. Shows total count.

**Sorting:** Client-side on current page (server data is already ordered by `trade_date` desc).

## Frontend Files

### Types (`lib/types/brokerage.ts`)

```typescript
interface BrokerageTransaction {
  id: string;
  accountId: string;
  snaptradeId: string;
  transactionType: string;       // BUY, SELL, DIVIDEND, etc.
  symbolTicker: string | null;
  symbolDescription: string | null;
  symbolCurrency: string | null;
  optionType: string | null;
  price: number;
  units: number;
  amount: number | null;
  currency: string;
  fee: number;
  fxRate: number | null;
  institution: string;
  description: string;
  tradeDate: string | null;
  settlementDate: string;
  externalReferenceId: string | null;
  rawJson: string;
  createdAt: string;
  updatedAt: string;
}

interface BrokerageSyncResult {
  transactionsSynced: number;
  holdingsSynced: number;
  balancesSynced: number;
}

interface BrokerageTransactionsPage {
  transactions: BrokerageTransaction[];
  total: number;
}

interface BrokerageFilters {
  symbol?: string;
  types?: string[];
  startDate?: string;
  endDate?: string;
  description?: string;
}
```

### Service (`lib/service/brokerage.ts`)

GraphQL operations:
- `fetchBrokerageTransactions(fetcher, accountId, offset, limit, filters)` — paginated query
- `syncBrokerageData(fetcher, accountId)` — mutation, returns `BrokerageSyncResult`
- `linkSnaptradeAccount(fetcher, accountId, snaptradeUserId, snaptradeUserSecret, connectionId)` — mutation

Field selection constant `BROKERAGE_TRANSACTION_FIELDS` to avoid duplication between queries.

### Hooks (`hooks/brokerage.ts`)

- `useBrokerageTransactions(accountId, offset, limit, filters)` — `useQuery` with key `["brokerage-transactions", accountId, offset, limit, filters]`
- `useSyncBrokerage(accountId)` — `useMutation` that invalidates `["brokerage-transactions"]` on success
- `useAutoSync(accountId)` — custom hook that calls `useSyncBrokerage` on mount (once per page load), tracks sync state (idle/syncing/synced/error) and last sync time
- `useLinkSnaptradeAccount()` — `useMutation`

## Filtering Strategy

All filters are **client-side** on the current page of data, except pagination which is server-side. This is pragmatic because:
- SnapTrade data is cached/refreshed daily anyway
- The backend query already supports `offset`/`limit`
- Type and symbol filtering across pages would require server-side filter params (can be added later if needed)

If the user needs to search across all pages, the re-sync + backend query approach handles it. Future enhancement: add server-side filter params to the GraphQL query.

## Error Handling

- **Sync failure** — toast error ("Failed to sync brokerage data"), sidebar shows red "Sync failed" status with retry button
- **Query failure** — error boundary with rose-themed error section (matches journal pattern)
- **No transactions after sync** — empty state within the table area: "No transactions found. Your brokerage may not have any transaction history yet."

## File Structure

```
frontend/src/
  app/dashboard/brokerage/
    page.tsx                     — page shell, state routing
  components/brokerage/
    brokerage-transactions.tsx   — connected state layout
    brokerage-filter-sidebar.tsx — filter panel
    brokerage-table.tsx          — data table + pagination
    brokerage-empty-state.tsx    — CTA card
  hooks/
    brokerage.ts                 — React Query hooks
  lib/types/
    brokerage.ts                 — TypeScript interfaces
  lib/service/
    brokerage.ts                 — GraphQL operations
```
