import { ScrollArea } from "../user-interface";
import JournalSidebar, { journalPageLabel } from "./sidebar";
import Dashboard from "./dashboard";

type JournalLayoutProps = {
  page?: string;
  onPageChange?: (id: string) => void;
};

export default function JournalLayout({ page, onPageChange }: JournalLayoutProps) {
  return (
    <div className="flex min-h-0 flex-1">
      <JournalSidebar active={page} onActiveChange={onPageChange} />
      <main className="min-h-0 flex-1 bg-zinc-50/95 dark:bg-zinc-950/90">
        <ScrollArea className="h-full">
          <div className="flex flex-col gap-4 p-6">
            <h1 className="text-xl font-semibold text-zinc-900 dark:text-zinc-50">
              {page ? journalPageLabel(page) : ""}
            </h1>
            {page === "dashboard" ? (
              <Dashboard />
            ) : (
              <p className="text-sm text-zinc-400 dark:text-zinc-600">
                Design coming soon
              </p>
            )}
          </div>
        </ScrollArea>
      </main>
    </div>
  );
}
