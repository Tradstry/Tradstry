"use client";

import { TradstryMark } from "@/components/logo";
import { Button } from "@/components/ui/button";
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { useChatStore } from "@/hooks/chat";
import { useNotebookPanelStore } from "@/hooks/notebook-panel";

export function ChatButton() {
  const toggleOpen = useChatStore((s) => s.toggleOpen);
  const closeNotes = useNotebookPanelStore((s) => s.setOpen);
  const isOpen = useChatStore((s) => s.isOpen);

  function handleClick() {
    closeNotes(false);
    toggleOpen();
  }

  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant="ghost"
          size="icon"
          className={isOpen ? "bg-muted text-foreground" : undefined}
          // Icon-only, so the name has to live somewhere a screen reader can reach.
          aria-label="Tradstry AI"
          aria-pressed={isOpen}
          onClick={handleClick}
        >
          <TradstryMark className="size-5" />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">Tradstry AI</TooltipContent>
    </Tooltip>
  );
}
