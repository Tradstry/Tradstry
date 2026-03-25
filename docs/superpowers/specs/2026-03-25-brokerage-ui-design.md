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

1. **Empty** — no brokerage linked on the active account (`activeAccount.snaptradeUserId == null`). Centered CTA card: icon, title ("Connect your brokerage"), description, "Connect Account" button, subtitle ("Supports 20+ brokerages via SnapTrade").
2. **Loading** — auto-sync in progress on page load. Skeleton sidebar + skeleton table rows + yellow "Syncing..." indicator in sidebar + indeterminate progress bar above table.
3. **Connected** — sidebar filters + transaction table with pagination.

### Linked Account Detection

The page checks `activeAccount?.snaptradeUserId != null` to determine empty vs connected state. The `Account` type must include the `snaptradeUserId` field (added during the backend brokerage integration).

## Layout

Sidebar-filter layout (matches existing dashboard patterns):

```
┌──────────────────────────────────────────────┐
│ SiteHeader: "Brokerage"                      │
├────────────┬─────────────────────────────────┤
│  Sidebar   │  Main content                   │
│  (200px)   │                                 │
│            │  [progress bar — indeterminate]  │
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

### Page Shell

Must mirror the journal page structure: `GraphQLProvider` → `ChatProvider` → `SidebarProvider` → `AppSidebar` → `SidebarInset` → `SiteHeader("Brokerage")` → content.

## Components

### `BrokeragePage` (`app/dashboard/brokerage/page.tsx`)

Top-level page component. Uses `useActiveAccount()` to check `snaptradeUserId`. Renders either `BrokerageEmptyState` or `BrokerageTransactions`.

### `BrokerageEmptyState`

Centered card using the existing `Empty` component pattern (icon, title, description, action button). "Connect Account" button triggers the SnapTrade connection flow (calls `linkSnaptradeAccount` mutation after portal redirect).

### `BrokerageTransactions`

Main connected view. Contains:
- `BrokerageFilterSidebar` — left panel with all filter controls
- `BrokerageTable` — right panel with data table + pagination

Manages shared state: `TransactionFilters` (server-side) + client-side `symbolSearch` and `descriptionSearch`.

### `BrokerageFilterSidebar`

Persistent left panel (200px, `border-r`, `bg-muted/30`).

Contents:
- **Sync status** — green "Synced" card with "Last: X min ago" or yellow "Syncing..." during auto-sync, or red "Sync failed" with retry
- **Symbol filter** — text input, client-side filter on current page against `symbol` and `rawSymbol` fields. Shows "Filtering current page" helper text.
- **Type filter** — toggle badges grouped by category:
  - **Trading:** BUY (emerald), SELL (rose)
  - **Income:** DIVIDEND, STOCK_DIVIDEND, INTEREST, REI (indigo)
  - **Options:** OPTIONEXPIRATION, OPTIONASSIGNMENT, OPTIONEXERCISE (violet)
  - **Transfers:** TRANSFER, CONTRIBUTION, WITHDRAWAL, EXTERNAL_ASSET_TRANSFER_IN/OUT (amber)
  - **Other:** FEE, TAX, SPLIT, ADJUSTMENT (gray)

  Type filter is **server-side** — passed as `transactionType` to the GraphQL query. When a single type is selected, it's sent to the server. When "All" is selected (default), no filter is sent.
- **Date range** — two date inputs (from/to), **server-side** via `startDate`/`endDate` params
- **Description search** — text input, **client-side** filter on current page. Shows "Filtering current page" helper text.
- **Clear all filters** — resets all to default
- **Re-sync button** — manual trigger for `syncBrokerageData`, below a separator

### `BrokerageTable`

Uses `@tanstack/react-table` following the journal-table pattern.

**Columns:**

| Column | Source field | Display |
|--------|-------------|---------|
| Date | `tradeDate` | Short date format (Mar 22). Sortable, default descending. |
| Symbol | `symbol` + `symbolDescription` | Ticker bold + description muted, truncated. "—" when null. |
| Type | `transactionType` | Colored badge per category (see type filter groups above) |
| Qty | `units` | Number, "—" when null/zero (dividends) |
| Price | `price` | Currency formatted |
| Amount | `amount` | Signed currency. Negative = red, positive = green. "—" when null. |
| Fee | `fee` | Currency formatted, muted when zero |

**Pagination:** Server-side via GraphQL `offset`/`limit` (inside `TransactionFilters`). Options: 20, 50, 100 per page. Shows total count from response.

**Sorting:** Client-side on current page (server data already ordered by `trade_date` desc).

## Existing Frontend Code (already implemented)

The types, service, and hooks layers are already written. The UI components are what need to be built.

### Types (`lib/types/brokerage.ts`) — EXISTS

Key interfaces: `BrokerageTransaction`, `BrokerageTransactionsPage` (with `data`, `total`, `offset`, `limit`), `BrokerageHolding`, `BrokerageBalance`, `TransactionFilters` (with `startDate`, `endDate`, `transactionType`, `offset`, `limit`), `SyncResult`, `LinkSnaptradeInput`.

Also exports `TRANSACTION_TYPES` const array (18 types) and `TransactionType` union type.

### Service (`lib/service/brokerage.ts`) — EXISTS

Functions:
- `fetchTransactions(fetcher, accountId, filters?)` → `BrokerageTransactionsPage`
- `fetchTransaction(fetcher, id)` → `BrokerageTransaction | null`
- `fetchHoldings(fetcher, accountId)` → `BrokerageHolding[]`
- `fetchBalances(fetcher, accountId)` → `BrokerageBalance[]`
- `linkSnaptradeAccount(fetcher, input)` → `boolean`
- `syncBrokerageData(fetcher, accountId)` → `SyncResult`

### Hooks (`hooks/brokerage.ts`) — EXISTS

- `useBrokerageTransactions(accountId, filters?)` — query key `["brokerage-transactions", accountId, filters]`
- `useBrokerageHoldings(accountId)` — query key `["brokerage-holdings", accountId]`
- `useBrokerageBalances(accountId)` — query key `["brokerage-balances", accountId]`
- `useSyncBrokerageData()` — mutation, invalidates all three query keys on success
- `useLinkSnaptradeAccount()` — mutation, invalidates `["accounts"]` on success

All query hooks include `enabled: isLoaded && isSignedIn && !!accountId` guard.

### Hook to add: `useAutoSync`

New hook needed in `hooks/brokerage.ts`:
- Calls `useSyncBrokerageData().mutateAsync(accountId)` on mount
- Uses a `useRef` flag to prevent double-fire in React strict mode
- Checks `sessionStorage` for last sync timestamp — skips if synced within last 5 minutes
- Returns `{ syncState: 'idle' | 'syncing' | 'synced' | 'error', lastSyncTime: string | null, retrySync: () => void }`
- Updates `sessionStorage` timestamp on successful sync

## Filtering Strategy

**Server-side filters** (sent to GraphQL via `TransactionFilters`):
- `transactionType` — single type string
- `startDate` / `endDate` — ISO date strings
- `offset` / `limit` — pagination

**Client-side filters** (applied to current page results):
- Symbol search — filters `data` array where `symbol` or `rawSymbol` contains search string (case-insensitive)
- Description search — filters `data` array where `description` contains search string (case-insensitive)
- Both show "Filtering current page" helper text so user understands the scope

## Error Handling

- **Sync failure** — toast error ("Failed to sync brokerage data"), sidebar shows red "Sync failed" status with retry button
- **Query failure** — error boundary with rose-themed error section (matches journal pattern)
- **No transactions after sync** — empty state within the table area: "No transactions found. Your brokerage may not have any transaction history yet."

## File Structure

```
frontend/src/
  app/dashboard/brokerage/
    page.tsx                     — page shell, state routing (TO BUILD)
  components/brokerage/
    brokerage-transactions.tsx   — connected state layout (TO BUILD)
    brokerage-filter-sidebar.tsx — filter panel (TO BUILD)
    brokerage-table.tsx          — data table + pagination (TO BUILD)
    brokerage-empty-state.tsx    — CTA card (TO BUILD)
  hooks/
    brokerage.ts                 — React Query hooks (EXISTS, add useAutoSync)
  lib/types/
    brokerage.ts                 — TypeScript interfaces (EXISTS)
  lib/service/
    brokerage.ts                 — GraphQL operations (EXISTS)
```
