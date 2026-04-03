"use client"

import { Button } from "@/components/ui/button"
import { HugeiconsIcon } from "@hugeicons/react"
import { AiChat02Icon } from "@hugeicons/core-free-icons"
import { useChatStore } from "@/hooks/chat"

export function ChatButton() {
  const toggleOpen = useChatStore((s) => s.toggleOpen)

  return (
    <Button variant="outline" size="sm" onClick={toggleOpen}>
      <HugeiconsIcon icon={AiChat02Icon} strokeWidth={2} className="size-4" />
      Chat AI
    </Button>
  )
}
