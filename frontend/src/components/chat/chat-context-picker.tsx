"use client";

import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs";
import { useChatStore } from "@/hooks/chat";

interface ChatContextPickerProps {
  onClose: () => void;
}

export function ChatContextPicker({ onClose }: ChatContextPickerProps) {
  const { setPinnedContext, pinnedContext } = useChatStore();
  const [from, setFrom] = useState(pinnedContext.dateRange?.from ?? "");
  const [to, setTo] = useState(pinnedContext.dateRange?.to ?? "");

  function applyDateRange() {
    if (from && to) {
      setPinnedContext({ ...pinnedContext, dateRange: { from, to } });
    }
    onClose();
  }

  return (
    <div className="absolute bottom-full left-0 z-50 mb-2 w-72 rounded-lg border border-border bg-background shadow-md">
      <Tabs defaultValue="date-range">
        <div className="flex items-center justify-between border-b border-border px-3 py-2">
          <TabsList variant="line" className="h-6">
            <TabsTrigger value="date-range">Date Range</TabsTrigger>
            <TabsTrigger value="trades">Trades</TabsTrigger>
            <TabsTrigger value="playbooks">Playbooks</TabsTrigger>
          </TabsList>
        </div>

        <TabsContent value="date-range" className="p-3">
          <div className="flex flex-col gap-2">
            <div className="flex flex-col gap-1">
              <label className="text-xs text-muted-foreground">From</label>
              <input
                type="date"
                value={from}
                onChange={(e) => setFrom(e.target.value)}
                className="h-7 rounded border border-input bg-input/20 px-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring dark:bg-input/30"
              />
            </div>
            <div className="flex flex-col gap-1">
              <label className="text-xs text-muted-foreground">To</label>
              <input
                type="date"
                value={to}
                onChange={(e) => setTo(e.target.value)}
                className="h-7 rounded border border-input bg-input/20 px-2 text-xs text-foreground focus:outline-none focus:ring-1 focus:ring-ring dark:bg-input/30"
              />
            </div>
            <Button
              size="sm"
              className="mt-1 w-full"
              onClick={applyDateRange}
              disabled={!from || !to}
            >
              Apply
            </Button>
          </div>
        </TabsContent>

        <TabsContent value="trades" className="p-3">
          <p className="text-xs text-muted-foreground">Trade filtering coming soon.</p>
        </TabsContent>

        <TabsContent value="playbooks" className="p-3">
          <p className="text-xs text-muted-foreground">Playbook filtering coming soon.</p>
        </TabsContent>
      </Tabs>
    </div>
  );
}
