# Brokerage Management Modal — Design Spec

**Date:** 2026-04-03

## Overview

Add a brokerage management button to the site header (left of the "Chat AI" button) that opens a modal for users to manage brokerage connections across all their accounts, view balances, sync data, and disconnect.

## Header Button

- New `<BrokerageButton />` component in `frontend/src/components/brokerage.tsx`
- Placed in `site-header.tsx`, to the left of the existing "Chat AI" button
- Uses `BankIcon` from `@hugeicons/core-free-icons` (same as sidebar)
- Owns the Dialog open/close state internally
- `frontend/src/components/chat.tsx` gets a matching `<ChatButton />` that wraps the existing chat toggle logic, keeping the header clean

## Modal Layout

Uses shadcn `Dialog` with `max-w-2xl`. Two-panel layout:

### Left Panel (~200px, scrollable)

- Lists all user accounts from `useAccounts()`
- Each row shows:
  - Account icon
  - Account name
  - Status dot: green if `snaptradeConnectionId` is set, gray otherwise
- Clicking a row selects that account (highlighted)
- First account is auto-selected on open

### Right Panel (remaining space)

**Connected account** (`snaptradeConnectionId` is set):

- Connection status badge showing broker name (from `account.broker`)
- Balance cards: one per currency, showing cash and buying power
  - Fetched via existing `useBrokerageBalances(account.id)`
- "Sync Now" button — calls `useSyncBrokerageData`
- "Disconnect" button — calls `useDisconnectBrokerage` with a confirmation step (window.confirm or inline)

**Unconnected account** (`snaptradeConnectionId` is null):

- Message: "No brokerage linked to this account"
- "Connect Account" button
  - Calls `useInitiateConnection` with `customRedirect` pointing to `/dashboard/brokerage/callback?accountId={id}`
  - Redirects browser to `portal.redirectUrl` (SnapTrade OAuth)
  - Reuses existing callback flow — no changes needed

## Component Structure

All in `frontend/src/components/brokerage.tsx`:

- **`BrokerageButton`** — Exported. Header button + Dialog state.
- **`BrokerageModal`** — Dialog content. Renders left + right panels.
- **`AccountList`** — Left panel. Maps `useAccounts()`, handles selection state.
- **`AccountDetail`** — Right panel. Conditionally renders connected/unconnected UI.

`frontend/src/components/chat.tsx`:

- **`ChatButton`** — Exported. Wraps existing chat toggle (`useChatStore.toggleOpen`) in a button component matching the header pattern.

## Reused Hooks & Services (no new backend work)

- `useAccounts()` from `@/components/accounts`
- `useBrokerageBalances(accountId)` from `@/hooks/brokerage`
- `useInitiateConnection()` from `@/hooks/brokerage`
- `useDisconnectBrokerage()` from `@/hooks/brokerage`
- `useSyncBrokerageData()` from `@/hooks/brokerage`

## OAuth Flow

Same as existing `BrokerageEmptyState`:

1. User clicks "Connect Account" in modal
2. `initiateConnection` mutation called with callback URL
3. Browser redirects to SnapTrade portal
4. After OAuth, redirects to `/dashboard/brokerage/callback`
5. Callback page completes the connection
6. User reopens modal to see updated status

## Out of Scope

- Multiple brokerage connections per single account
- Showing holdings summaries in the modal
- Opening SnapTrade in a new tab/window
- Backend schema changes
