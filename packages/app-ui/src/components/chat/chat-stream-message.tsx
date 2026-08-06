"use client";

import { Loading01Icon } from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { redactInternalIds } from "@tradstry/app-ui/lib/types/chat";

interface ChatStreamMessageProps {
  content: string;
  isStreaming: boolean;
}

function cleanContent(text: string): string {
  return redactInternalIds(text)
    .replace(/\*\*/g, "")
    .replace(/\*/g, "")
    .replace(/[—–]/g, "-")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\|[-\s|:]+\|$/gm, "")
    .replace(/\|/g, "  ")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

export function ChatStreamMessage({
  content,
  isStreaming,
}: ChatStreamMessageProps) {
  return (
    <div className="flex justify-start">
      <div className="max-w-[80%]">
        {content ? (
          <div className="whitespace-pre-wrap rounded-lg bg-muted px-4 py-3 text-xs/relaxed text-foreground">
            {cleanContent(content)}
            {isStreaming && (
              <span className="ml-0.5 inline-block animate-pulse">&#9612;</span>
            )}
          </div>
        ) : (
          <div className="rounded-lg bg-muted px-4 py-3 text-xs/relaxed">
            <span className="flex items-center gap-1.5 text-muted-foreground">
              <HugeiconsIcon
                icon={Loading01Icon}
                className="size-3 animate-spin"
              />
              Thinking...
            </span>
          </div>
        )}
      </div>
    </div>
  );
}
