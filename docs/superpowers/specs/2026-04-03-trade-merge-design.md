# Trade Merge to Journal — Design Spec

**Date:** 2026-04-03

## Overview

Allow users to select multiple brokerage transactions from the brokerage table, merge them into a single journal entry with auto-calculated fields, fill in required analysis fields, and create the journal entry with a link back to the source trades.

## Brokerage Table — Selection UI

- New **checkbox column** as the first column in the brokerage table.
- Header checkbox toggles all visible rows on the current page.
- Trades already linked to a journal entry show a **"Journaled" badge** and their checkbox is **disabled**.
- When 2+ trades of the **same symbol** are selected, a **floating action bar** appears at the bottom of the table:
  - Shows: "{count} {SYMBOL} trades selected"
  - "Merge to Journal" button
- If selected trades have **mixed symbols**, the merge button is disabled with tooltip: "Select trades of the same symbol."
- Selection state is local to the component (not persisted).

## Merge Modal

Opens when user clicks "Merge to Journal". Uses the same Dialog pattern as the existing `CreateTrades` component.

### Top Section — Selected Trades Summary

A compact read-only list of the selected brokerage transactions showing: date, transaction type (BUY/SELL), units, price. Confirms what the user is merging.

### Bottom Section — Journal Entry Form

All fields are editable. Auto-calculated fields are pre-filled from the selected trades:

| Field | Auto-calculated from | Default |
|-------|---------------------|---------|
| Symbol | Selected trades (shared symbol) | Pre-filled |
| Symbol Name | First trade's `symbolDescription` | Pre-filled |
| Open Date | Earliest `tradeDate` among selected | Pre-filled |
| Close Date | Latest `tradeDate` among selected | Pre-filled |
| Entry Price | Weighted average of BUY prices by units | Pre-filled |
| Exit Price | Weighted average of SELL prices by units | Pre-filled |
| Position Size | Total units from BUY transactions | Pre-filled |
| Trade Type | First chronological trade is BUY = "long", SELL = "short" | Pre-filled |
| Stop Loss | — | Empty (required) |
| Mistakes | — | Empty (required) |
| Entry Tactics | — | Empty (required) |
| Edges Spotted | — | Empty (required) |
| Playbook | — | Empty (optional) |
| Notes | — | Empty (optional) |

### Calculation Details

- **Weighted average price**: `sum(price * abs(units)) / sum(abs(units))` for BUY and SELL groups separately.
- **Position size**: `sum(abs(units))` for BUY transactions.
- **Trade type detection**: Sort selected trades by `tradeDate` ascending. If the first trade's `transactionType` is "BUY", trade type is "long". If "SELL", trade type is "short".

### Submit

Calls `createJournalEntry` mutation with all form fields plus `brokerageTransactionIds: [String!]` containing the IDs of selected brokerage transactions.

## Backend — Linking Trades to Journal Entries

### New DB Table: `journal_brokerage_links`

```sql
CREATE TABLE IF NOT EXISTS journal_brokerage_links (
    id TEXT PRIMARY KEY,
    journal_entry_id TEXT NOT NULL,
    brokerage_transaction_id TEXT NOT NULL UNIQUE,
    user_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (journal_entry_id) REFERENCES journal_entries(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_jbl_journal_entry ON journal_brokerage_links(journal_entry_id);
CREATE INDEX IF NOT EXISTS idx_jbl_user ON journal_brokerage_links(user_id);
```

- Unique constraint on `brokerage_transaction_id` prevents double-linking.
- `ON DELETE CASCADE` on `journal_entry_id` FK — deleting a journal entry frees the brokerage trades for re-merging.

### GraphQL Changes

**Mutation — `createJournalEntry`:**
- New optional input field: `brokerageTransactionIds: [String!]`
- After inserting the journal entry, insert rows into `journal_brokerage_links` for each transaction ID.
- If any transaction ID is already linked (unique constraint violation), return an error.

**New Query — `linkedBrokerageTransactionIds`:**
- Input: `accountId: String!`
- Returns: `[String!]!` — all `brokerage_transaction_id` values linked for the current user + account.
- Used by the frontend to mark rows as "Journaled" and disable their checkboxes.

**Existing — `deleteJournalEntry`:**
- No changes needed. The `ON DELETE CASCADE` handles link cleanup automatically.

### Frontend Hooks

- New hook: `useLinkedBrokerageTransactionIds(accountId)` — calls the new query.
- Modify existing `useCreateJournalEntry` — the mutation input type already passes through to GraphQL, just needs the new optional `brokerageTransactionIds` field added to the TypeScript type.

## Component Structure

All new components in `frontend/src/components/brokerage/`:

- **`brokerage-table.tsx`** — Modified: add checkbox column, selection state, floating action bar, "Journaled" badge on linked rows.
- **`merge-trades-modal.tsx`** — New: the merge dialog with trade summary + journal entry form. Handles auto-calculation and form submission.

## Validation

- Must select 2+ trades.
- All selected trades must share the same symbol.
- No selected trade can already be linked to a journal entry.
- All required journal fields must be filled (stop loss, mistakes, entry tactics, edges spotted).
- Stop loss validation: below entry price for longs, above for shorts.

## Out of Scope

- Editing the linked brokerage transactions after merge (e.g., adding/removing trades from an existing journal entry).
- Viewing linked brokerage transactions from the journal page.
- Bulk operations (merging multiple groups at once).
