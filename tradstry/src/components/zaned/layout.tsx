import { ScrollArea, Sidebar } from "../user-interface";
import AnalyticsPanel from "../analytics/analytics-panel";
import TradesTable from "../TradesTable";

export default function ZanedLayout() {
  return (
    <div className="flex min-h-0 flex-1">
      <main className="min-h-0 flex-1 bg-zinc-50/95 dark:bg-zinc-950/90">
        <ScrollArea className="h-full">
          <div className="flex flex-col gap-4 p-6">
            <h1 className="text-xl font-semibold text-zinc-900 dark:text-zinc-50">
              Overview
            </h1>
            <AnalyticsPanel />
            <h2 className="mt-2 text-sm font-semibold text-zinc-700 dark:text-zinc-300">
              Trades
            </h2>
            <TradesTable />
          </div>
        </ScrollArea>
      </main>
      <Sidebar />
    </div>
  );
}
