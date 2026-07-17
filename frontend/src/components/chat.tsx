"use client";

import { SparklesIcon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
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
          variant="outline"
          size="icon"
          // Icon-only, so the name has to live somewhere a screen reader can reach.
          aria-label="Chat AI"
          aria-pressed={isOpen}
          onClick={handleClick}
        >
          <HugeiconsIcon
            icon={SparklesIcon}
            className="size-5"
            strokeWidth={2}
          />
        </Button>
      </TooltipTrigger>
      <TooltipContent side="bottom">Chat AI</TooltipContent>
    </Tooltip>
  );
}
