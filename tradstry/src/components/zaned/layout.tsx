import { Sidebar } from "../user-interface";
import AnalyticsPanel from "../analytics/analytics-panel";
import TradesTable from "../TradesTable";

export default function ZanedLayout() {
  return (
    <div className="flex min-h-0 flex-1">
      <main className="flex flex-1 flex-col gap-4 overflow-auto overscroll-none bg-zinc-50/95 p-6 dark:bg-zinc-950/90">
        <h1 className="text-xl font-semibold text-zinc-900 dark:text-zinc-50">
          Overview
        </h1>
        <AnalyticsPanel />
        <h2 className="mt-2 text-sm font-semibold text-zinc-700 dark:text-zinc-300">
          Trades
        </h2>
        <TradesTable />
      </main>
      <Sidebar />
    </div>
  );
}
