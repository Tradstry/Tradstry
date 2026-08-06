import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { ScrollArea } from "../user-interface";
import JournalSidebar, { journalPageLabel } from "./sidebar";
import Dashboard from "./dashboard";
import PlaybookView from "./playbook";
import JournalTrades from "./trades-table";
import AnalyticsView from "./analytics";
import NotebookView from "./notebook";

type JournalLayoutProps = {
  page?: string;
  onPageChange?: (id: string) => void;
};

export default function JournalLayout({ page, onPageChange }: JournalLayoutProps) {
  const reduce = useReducedMotion();
  return (
    <div className="flex min-h-0 flex-1">
      <JournalSidebar active={page} onActiveChange={onPageChange} />
      <main className="min-h-0 flex-1 bg-zinc-50/95 dark:bg-zinc-950/90">
        {page === "notebook" ? (
          <NotebookView />
        ) : (
        <ScrollArea className="h-full">
          <div className="p-6">
            <AnimatePresence mode="wait">
              <motion.div
                key={page}
                className="flex flex-col gap-4"
                initial={reduce ? { opacity: 0 } : { opacity: 0, y: 10 }}
                animate={{ opacity: 1, y: 0 }}
                exit={reduce ? { opacity: 0 } : { opacity: 0, y: -8 }}
                transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
              >
                <h1 className="text-xl font-semibold text-zinc-900 dark:text-zinc-50">
                  {page ? journalPageLabel(page) : ""}
                </h1>
                {page === "dashboard" ? (
                  <Dashboard />
                ) : page === "playbook" ? (
                  <PlaybookView />
                ) : page === "journal" ? (
                  <JournalTrades />
                ) : page === "analytics" ? (
                  <AnalyticsView />
                ) : (
                  <p className="text-sm text-zinc-400 dark:text-zinc-600">
                    Design coming soon
                  </p>
                )}
              </motion.div>
            </AnimatePresence>
          </div>
        </ScrollArea>
        )}
      </main>
    </div>
  );
}
