import { PlusIcon } from "@phosphor-icons/react";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";

type AddButtonProps = React.ComponentProps<typeof Button> & { label: string };

/** Quiet "+" for the notebook sidebar headers: muted until hovered, then it
 *  picks up the accent. Shared so the folder and note headers stay identical. */
export function AddButton({ label, className, ...props }: AddButtonProps) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          size="icon-sm"
          variant="ghost"
          aria-label={label}
          className={cn(
            "relative text-zinc-500 hover:bg-blue-500/10 hover:text-blue-600 active:bg-blue-500/15 dark:text-zinc-400 dark:hover:bg-blue-500/15 dark:hover:text-blue-400",
            // Grows the hit area past the 28px glyph without shifting layout.
            "after:absolute after:-inset-1 after:content-['']",
            className,
          )}
          {...props}
        >
          <PlusIcon size={16} weight="bold" />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">{label}</TooltipContent>
    </Tooltip>
  );
}
