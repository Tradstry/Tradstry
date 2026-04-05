export function Footer() {
  return (
    <footer className="border-t border-white/5 bg-black py-8">
      <div className="mx-auto flex max-w-6xl items-center justify-between px-6">
        <span className="text-sm font-medium text-zinc-400">Tradstry</span>
        <span className="text-sm text-zinc-600">
          &copy; {new Date().getFullYear()} Tradstry. All rights reserved.
        </span>
      </div>
    </footer>
  );
}
