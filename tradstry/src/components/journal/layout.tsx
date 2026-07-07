import JournalSidebar from "./sidebar";

export default function JournalLayout() {
  return (
    <div className="flex min-h-0 flex-1">
      <JournalSidebar />
      <main className="flex flex-1 items-center justify-center overscroll-none bg-zinc-50/95 dark:bg-zinc-950/90">
        <p className="text-sm text-zinc-400 dark:text-zinc-600">
          Design coming soon
        </p>
      </main>
    </div>
  );
}
