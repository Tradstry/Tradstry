import {
  CheckCircleIcon,
  InfoIcon,
  WarningCircleIcon,
  XIcon,
  type Icon,
} from "@phosphor-icons/react";
import { toast } from "sonner";
import { cn } from "@/lib/utils";

type Variant = "success" | "error" | "info";

const VARIANTS: Record<
  Variant,
  { Icon: Icon; tint: string; bg: string; bar: string }
> = {
  success: {
    Icon: CheckCircleIcon,
    tint: "text-zinc-700 dark:text-zinc-200",
    bg: "bg-zinc-500/15",
    bar: "bg-zinc-400 dark:bg-zinc-600",
  },
  error: {
    Icon: WarningCircleIcon,
    tint: "text-red-600 dark:text-red-400",
    bg: "bg-red-500/15",
    bar: "bg-red-500",
  },
  info: {
    Icon: InfoIcon,
    tint: "text-blue-600 dark:text-blue-400",
    bg: "bg-blue-500/15",
    bar: "bg-blue-500",
  },
};

function ToastCard({
  variant,
  message,
  description,
  onDismiss,
}: {
  variant: Variant;
  message: string;
  description?: string;
  onDismiss: () => void;
}) {
  const { Icon, tint, bg, bar } = VARIANTS[variant];
  return (
    <div className="relative flex w-[356px] items-start gap-3 overflow-hidden rounded-xl border border-black/5 bg-white/85 p-3.5 pr-9 shadow-lg ring-1 ring-black/5 backdrop-blur-xl dark:border-white/10 dark:bg-zinc-900/85 dark:ring-white/10">
      <span
        className={cn("absolute inset-y-2.5 left-0 w-1 rounded-full", bar)}
        aria-hidden="true"
      />
      <span
        className={cn(
          "mt-px flex size-6 shrink-0 items-center justify-center rounded-full",
          bg,
          tint,
        )}
      >
        <Icon size={16} weight="fill" />
      </span>
      <div className="min-w-0 flex-1 pt-0.5">
        <p className="text-sm font-medium text-zinc-900 dark:text-zinc-50">
          {message}
        </p>
        {description && (
          <p className="mt-0.5 text-xs leading-snug text-zinc-500 dark:text-zinc-400">
            {description}
          </p>
        )}
      </div>
      <button
        type="button"
        onClick={onDismiss}
        aria-label="Dismiss notification"
        className="absolute right-2 top-2 flex size-6 cursor-pointer items-center justify-center rounded-md text-zinc-400 transition-colors hover:bg-zinc-100 hover:text-zinc-700 dark:hover:bg-zinc-800 dark:hover:text-zinc-200"
      >
        <XIcon size={13} />
      </button>
    </div>
  );
}

function show(variant: Variant, message: string, description?: string) {
  return toast.custom(
    (id) => (
      <ToastCard
        variant={variant}
        message={message}
        description={description}
        onDismiss={() => toast.dismiss(id)}
      />
    ),
    // `unstyled` drops sonner's default banner chrome so our card is the toast.
    { unstyled: true },
  );
}

/** App-wide toast — a custom liquid-glass card, not sonner's default banner. */
export const notify = {
  success: (message: string, description?: string) =>
    show("success", message, description),
  error: (message: string, description?: string) =>
    show("error", message, description),
  info: (message: string, description?: string) =>
    show("info", message, description),
};
