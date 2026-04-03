# Brokerage Management Modal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a brokerage management button to the site header that opens a list+detail modal for managing brokerage connections, viewing balances, syncing, and disconnecting — per account.

**Architecture:** Two new components (`BrokerageButton` in `brokerage.tsx`, `ChatButton` in `chat.tsx`) rendered in the site header. The modal uses shadcn Dialog with a two-panel layout. All data fetching reuses existing hooks — no backend changes.

**Tech Stack:** React, shadcn Dialog, @hugeicons, TanStack Query (existing hooks), Zustand (existing chat store)

---

### Task 1: Create `ChatButton` component

**Files:**
- Modify: `frontend/src/components/chat.tsx`

This extracts the existing chat button logic from the header into its own component, so the header can render both buttons cleanly.

- [ ] **Step 1: Write the ChatButton component**

Replace the contents of `frontend/src/components/chat.tsx` with:

```tsx
"use client"

import { Button } from "@/components/ui/button"
import { HugeiconsIcon } from "@hugeicons/react"
import { AiChat02Icon } from "@hugeicons/core-free-icons"
import { useChatStore } from "@/hooks/chat"

export function ChatButton() {
  const toggleOpen = useChatStore((s) => s.toggleOpen)

  return (
    <Button variant="outline" size="sm" onClick={toggleOpen}>
      <HugeiconsIcon icon={AiChat02Icon} strokeWidth={2} className="size-4" />
      Chat AI
    </Button>
  )
}
```

- [ ] **Step 2: Update the site header to use ChatButton**

In `frontend/src/components/site-header.tsx`, replace the inline chat button with the new component.

Change the imports — remove `Button`, `HugeiconsIcon`, `AiChat02Icon`, `useChatStore`. Add `ChatButton`:

```tsx
"use client"

import { usePathname } from "next/navigation"
import { Separator } from "@/components/ui/separator"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { ChatButton } from "@/components/chat"

const ROUTE_TITLES: Record<string, string> = {
  "/dashboard": "Dashboard",
  "/dashboard/playbook": "Playbook",
  "/dashboard/journal": "Journal",
  "/dashboard/notebook": "Notebook",
  "/dashboard/brokerage": "Brokerage",
}

export function SiteHeader({ actions }: { actions?: React.ReactNode }) {
  const pathname = usePathname()
  const title = ROUTE_TITLES[pathname] ?? "Dashboard"

  return (
    <header className="flex h-(--header-height) shrink-0 items-center gap-2 border-b transition-[width,height] ease-linear group-has-data-[collapsible=icon]/sidebar-wrapper:h-(--header-height)">
      <div className="flex w-full items-center gap-1 px-4 lg:gap-2 lg:px-6">
        <SidebarTrigger className="-ml-1" />
        <Separator
          orientation="vertical"
          className="mx-2 data-[orientation=vertical]:h-4"
        />
        <h1 className="text-base font-medium">{title}</h1>
        {actions ? <div className="ml-4">{actions}</div> : null}
        <div className="ml-auto flex items-center gap-2">
          <ChatButton />
        </div>
      </div>
    </header>
  )
}
```

Key change: the `ml-auto` div now uses `flex items-center gap-2` to hold multiple buttons.

- [ ] **Step 3: Verify the app compiles**

Run: `cd frontend && npx next build --no-lint 2>&1 | tail -20` (or dev server check)
Expected: No build errors. Header should render identically to before.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/chat.tsx frontend/src/components/site-header.tsx
git commit -m "refactor: extract ChatButton component from site header"
```

---

### Task 2: Create `BrokerageButton` with empty modal shell

**Files:**
- Modify: `frontend/src/components/brokerage.tsx`
- Modify: `frontend/src/components/site-header.tsx`

- [ ] **Step 1: Write the BrokerageButton with a Dialog shell**

Replace the contents of `frontend/src/components/brokerage.tsx` with:

```tsx
"use client"

import { useState } from "react"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
} from "@/components/ui/dialog"
import { HugeiconsIcon } from "@hugeicons/react"
import { BankIcon } from "@hugeicons/core-free-icons"

export function BrokerageButton() {
  const [open, setOpen] = useState(false)

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <Button variant="outline" size="sm" onClick={() => setOpen(true)}>
        <HugeiconsIcon icon={BankIcon} strokeWidth={2} className="size-4" />
        Brokerage
      </Button>
      <DialogContent className="sm:max-w-2xl p-0 gap-0">
        <DialogHeader className="p-4 pb-0">
          <DialogTitle>Brokerage Connections</DialogTitle>
          <DialogDescription>
            Manage brokerage connections across your accounts.
          </DialogDescription>
        </DialogHeader>
        <div className="flex h-[400px] border-t mt-4">
          <div className="w-[200px] shrink-0 border-r p-2">
            {/* AccountList goes here */}
            <p className="text-xs text-muted-foreground p-2">Accounts</p>
          </div>
          <div className="flex-1 p-4">
            {/* AccountDetail goes here */}
            <p className="text-xs text-muted-foreground">Select an account</p>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
```

- [ ] **Step 2: Add BrokerageButton to the site header**

In `frontend/src/components/site-header.tsx`, add the import and render it before ChatButton:

Add import:
```tsx
import { BrokerageButton } from "@/components/brokerage"
```

In the `ml-auto` div, add `<BrokerageButton />` before `<ChatButton />`:

```tsx
<div className="ml-auto flex items-center gap-2">
  <BrokerageButton />
  <ChatButton />
</div>
```

- [ ] **Step 3: Verify the app compiles and the modal opens**

Run: `cd frontend && npx next build --no-lint 2>&1 | tail -20`
Expected: No build errors. Clicking the "Brokerage" button in the header opens a two-panel modal shell.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/brokerage.tsx frontend/src/components/site-header.tsx
git commit -m "feat: add brokerage button with empty modal shell in header"
```

---

### Task 3: Implement the AccountList (left panel)

**Files:**
- Modify: `frontend/src/components/brokerage.tsx`

- [ ] **Step 1: Add AccountList and wire it into the modal**

Update `frontend/src/components/brokerage.tsx`. Add imports at the top:

```tsx
import { useAccounts } from "@/components/accounts"
import type { Account } from "@/components/accounts"
```

Add the `AccountList` component (inside the same file, not exported):

```tsx
function AccountList({
  selectedId,
  onSelect,
}: {
  selectedId: string | null
  onSelect: (id: string) => void
}) {
  const accounts = useAccounts()

  if (accounts.length === 0) {
    return (
      <p className="p-3 text-xs text-muted-foreground">No accounts found.</p>
    )
  }

  return (
    <div className="flex flex-col gap-0.5">
      {accounts.map((account) => {
        const connected = !!account.snaptradeConnectionId
        const isSelected = account.id === selectedId
        return (
          <button
            key={account.id}
            onClick={() => onSelect(account.id)}
            className={`flex items-center gap-2 rounded-md px-2.5 py-2 text-left text-xs transition-colors ${
              isSelected
                ? "bg-accent text-accent-foreground"
                : "hover:bg-muted"
            }`}
          >
            <span className="text-base">{account.icon}</span>
            <span className="flex-1 truncate font-medium">{account.name}</span>
            <span
              className={`size-2 shrink-0 rounded-full ${
                connected ? "bg-emerald-500" : "bg-muted-foreground/30"
              }`}
            />
          </button>
        )
      })}
    </div>
  )
}
```

- [ ] **Step 2: Wire AccountList into BrokerageButton with selection state**

In `BrokerageButton`, add selection state and auto-select logic. Update imports to include `useEffect`:

```tsx
import { useState, useEffect } from "react"
```

Add to the `BrokerageButton` component body (after the `open` state):

```tsx
const accounts = useAccounts()
const [selectedId, setSelectedId] = useState<string | null>(null)

// Auto-select first account when modal opens
useEffect(() => {
  if (open && accounts.length > 0 && !selectedId) {
    setSelectedId(accounts[0].id)
  }
}, [open, accounts, selectedId])

// Reset selection when modal closes
useEffect(() => {
  if (!open) setSelectedId(null)
}, [open])
```

Replace the left panel placeholder in the JSX:

```tsx
<div className="w-[200px] shrink-0 border-r p-2 overflow-y-auto">
  <AccountList selectedId={selectedId} onSelect={setSelectedId} />
</div>
```

- [ ] **Step 3: Verify the app compiles**

Run: `cd frontend && npx next build --no-lint 2>&1 | tail -20`
Expected: No build errors. Opening the modal shows account list with status dots.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/brokerage.tsx
git commit -m "feat: add account list panel to brokerage modal"
```

---

### Task 4: Implement the AccountDetail (right panel)

**Files:**
- Modify: `frontend/src/components/brokerage.tsx`

- [ ] **Step 1: Add AccountDetail component**

Add these imports at the top of `frontend/src/components/brokerage.tsx`:

```tsx
import {
  useBrokerageBalances,
  useInitiateConnection,
  useDisconnectBrokerage,
  useSyncBrokerageData,
} from "@/hooks/brokerage"
import { toast } from "sonner"
```

Add the `AccountDetail` component (not exported):

```tsx
function AccountDetail({ account }: { account: Account }) {
  const connected = !!account.snaptradeConnectionId
  const { data: balances, isLoading: balancesLoading } = useBrokerageBalances(
    connected ? account.id : null,
  )
  const initiate = useInitiateConnection()
  const disconnect = useDisconnectBrokerage()
  const sync = useSyncBrokerageData()

  async function handleConnect() {
    try {
      const callbackUrl = `${window.location.origin}/dashboard/brokerage/callback?accountId=${account.id}`
      const portal = await initiate.mutateAsync({
        accountId: account.id,
        customRedirect: callbackUrl,
      })
      window.location.href = portal.redirectUrl
    } catch (err) {
      toast.error(
        `Failed to connect: ${err instanceof Error ? err.message : "Unknown error"}`,
      )
    }
  }

  async function handleDisconnect() {
    if (!confirm("Disconnect this brokerage? You can reconnect later.")) return
    try {
      await disconnect.mutateAsync(account.id)
      toast.success("Brokerage disconnected")
    } catch {
      toast.error("Failed to disconnect")
    }
  }

  async function handleSync() {
    try {
      const result = await sync.mutateAsync(account.id)
      toast.success(
        `Synced ${result.transactionsSynced} transactions, ${result.holdingsSynced} holdings`,
      )
    } catch {
      toast.error("Failed to sync")
    }
  }

  if (!connected) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 text-center">
        <div className="rounded-full bg-muted p-3">
          <HugeiconsIcon icon={BankIcon} strokeWidth={2} className="size-6 text-muted-foreground" />
        </div>
        <div>
          <p className="text-sm font-medium">No brokerage linked</p>
          <p className="mt-1 text-xs text-muted-foreground">
            Connect a brokerage to sync transactions and balances.
          </p>
        </div>
        <Button size="sm" onClick={handleConnect} disabled={initiate.isPending}>
          {initiate.isPending ? "Connecting..." : "Connect Account"}
        </Button>
      </div>
    )
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Connection status */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className="size-2 rounded-full bg-emerald-500" />
          <span className="text-xs font-medium">
            Connected{account.broker ? ` to ${account.broker}` : ""}
          </span>
        </div>
        <div className="flex items-center gap-1.5">
          <Button
            variant="outline"
            size="xs"
            onClick={handleSync}
            disabled={sync.isPending}
          >
            {sync.isPending ? "Syncing..." : "Sync Now"}
          </Button>
          <Button
            variant="outline"
            size="xs"
            onClick={handleDisconnect}
            disabled={disconnect.isPending}
            className="text-destructive hover:bg-destructive/10"
          >
            {disconnect.isPending ? "..." : "Disconnect"}
          </Button>
        </div>
      </div>

      {/* Balances */}
      <div>
        <h3 className="mb-2 text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Balances
        </h3>
        {balancesLoading ? (
          <p className="text-xs text-muted-foreground">Loading balances...</p>
        ) : !balances || balances.length === 0 ? (
          <p className="text-xs text-muted-foreground">
            No balance data. Try syncing.
          </p>
        ) : (
          <div className="grid gap-2">
            {balances.map((b) => (
              <div
                key={b.id}
                className="rounded-lg border p-3"
              >
                <p className="text-xs font-semibold">{b.currency}</p>
                <div className="mt-1.5 flex gap-6">
                  <div>
                    <p className="text-[0.6rem] uppercase text-muted-foreground">
                      Cash
                    </p>
                    <p className="text-sm font-medium tabular-nums">
                      {formatCurrency(b.cash, b.currency)}
                    </p>
                  </div>
                  <div>
                    <p className="text-[0.6rem] uppercase text-muted-foreground">
                      Buying Power
                    </p>
                    <p className="text-sm font-medium tabular-nums">
                      {formatCurrency(b.buyingPower, b.currency)}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        )}
      </div>
    </div>
  )
}

function formatCurrency(value: number | null | undefined, currency: string): string {
  if (value == null) return "—"
  return new Intl.NumberFormat("en-US", {
    style: "currency",
    currency,
    minimumFractionDigits: 2,
  }).format(value)
}
```

- [ ] **Step 2: Wire AccountDetail into the modal right panel**

In `BrokerageButton`, find the selected account and replace the right panel placeholder:

Add this after the `useEffect` hooks:

```tsx
const selectedAccount = accounts.find((a) => a.id === selectedId) ?? null
```

Replace the right panel div:

```tsx
<div className="flex-1 overflow-y-auto p-4">
  {selectedAccount ? (
    <AccountDetail account={selectedAccount} />
  ) : (
    <p className="text-xs text-muted-foreground">Select an account</p>
  )}
</div>
```

- [ ] **Step 3: Verify the app compiles**

Run: `cd frontend && npx next build --no-lint 2>&1 | tail -20`
Expected: No build errors. Modal shows connected/unconnected state per account with balances and action buttons.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/brokerage.tsx
git commit -m "feat: add account detail panel with balances and connection actions"
```

---

### Task 5: Final verification

**Files:** None (read-only check)

- [ ] **Step 1: Full build check**

Run: `cd frontend && npx next build --no-lint 2>&1 | tail -30`
Expected: Build succeeds with no errors.

- [ ] **Step 2: Verify complete flow**

Manually confirm (or via dev server):
1. Header shows "Brokerage" button to the left of "Chat AI"
2. Clicking "Brokerage" opens a two-panel modal
3. Left panel lists all accounts with green/gray status dots
4. Clicking an account shows its detail on the right
5. Connected accounts show balances, Sync Now, and Disconnect buttons
6. Unconnected accounts show "Connect Account" button
