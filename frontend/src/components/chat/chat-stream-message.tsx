"use client";

import { HugeiconsIcon } from "@hugeicons/react";
import { Loading01Icon } from "@hugeicons/core-free-icons";

interface ChatStreamMessageProps {
  content: string;
  toolName: string | null;
}

function cleanContent(text: string): string {
  return text
    .replace(/\*\*/g, "")
    .replace(/\*/g, "")
    .replace(/[—–]/g, "-")
    .replace(/^#{1,6}\s+/gm, "")
    .replace(/^\|[-\s|:]+\|$/gm, "")
    .replace(/\|/g, "  ")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

export function ChatStreamMessage({ content, toolName }: ChatStreamMessageProps) {
  return (
    <div className="flex justify-start">
      <div className="max-w-[80%] whitespace-pre-wrap rounded-lg bg-muted px-3 py-2 text-xs/relaxed text-foreground">
        {toolName ? (
          <span className="flex items-center gap-1.5 text-muted-foreground">
            <HugeiconsIcon icon={Loading01Icon} className="size-3 animate-spin" />
            Searching: {toolName}...
          </span>
        ) : content ? (
          <span>
            {cleanContent(content)}
            <span className="ml-0.5 inline-block animate-pulse">▌</span>
          </span>
        ) : (
          <span className="flex items-center gap-1.5 text-muted-foreground">
            <HugeiconsIcon icon={Loading01Icon} className="size-3 animate-spin" />
            Thinking...
          </span>
        )}
      </div>
    </div>
  );
}
