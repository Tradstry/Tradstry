"use client";

import { useActiveAccount } from "@/components/accounts";
import { TagManager } from "@/components/journal/tag-manager";
import { JournalTable } from "./journal-table";

export function Journal() {
  const activeAccount = useActiveAccount();

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-end">
        <TagManager />
      </div>
      <JournalTable key={activeAccount?.id ?? "no-account"} />
    </div>
  );
}
