"use client";

import { TagManager } from "@/components/journal/tag-manager";
import { useActiveWorkspace } from "@/components/workspaces";
import { JournalTable } from "./journal-table";

export function Journal() {
  const activeWorkspace = useActiveWorkspace();

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-end">
        <TagManager />
      </div>
      <JournalTable key={activeWorkspace?.id ?? "no-workspace"} />
    </div>
  );
}
