"use client";

import {
  Add01Icon,
  Cancel01Icon,
  Chatting01Icon,
  Delete02Icon,
  Menu11Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import {
  Drawer,
  DrawerClose,
  DrawerContent,
  DrawerDescription,
  DrawerHeader,
  DrawerTitle,
  DrawerTrigger,
} from "@/components/ui/drawer";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import type { AiChatThread } from "@/lib/types/ai_chat";
import { cn } from "@/lib/utils";

export function ManageChats({
  chats,
  selectedThreadId,
  disabled = false,
  isCreating = false,
  deletingThreadId = null,
  onCreateChat,
  onSelectChat,
  onDeleteChat,
}: {
  chats: AiChatThread[];
  selectedThreadId: string | null;
  disabled?: boolean;
  isCreating?: boolean;
  deletingThreadId?: string | null;
  onCreateChat: () => void;
  onSelectChat: (threadId: string) => void;
  onDeleteChat: (threadId: string) => void;
}) {
  const [confirmingThreadId, setConfirmingThreadId] = useState<string | null>(null);

  return (
    <Drawer direction="right">
      <DrawerTrigger asChild>
        <Button type="button" variant="outline" size="lg" disabled={disabled}>
          <HugeiconsIcon icon={Menu11Icon} strokeWidth={2} />
          Manage Chats
        </Button>
      </DrawerTrigger>
      <DrawerContent className="w-full max-w-md p-0 before:inset-0 before:rounded-none before:border-l">
        <DrawerClose asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-sm"
            className="absolute top-4 left-4 z-10"
          >
            <HugeiconsIcon icon={Cancel01Icon} strokeWidth={2} />
            <span className="sr-only">Close</span>
          </Button>
        </DrawerClose>
        <DrawerHeader className="border-b border-slate-200 px-6 py-5 pl-14">
          <DrawerTitle className="text-base font-semibold text-slate-950">
            Manage Chats
          </DrawerTitle>
          <DrawerDescription className="text-sm text-slate-500">
            Create a new thread, switch between saved chats, or delete old conversations.
          </DrawerDescription>
        </DrawerHeader>
        <div className="border-b border-slate-200 px-6 py-4">
          <Button
            type="button"
            className="w-full justify-center"
            onClick={onCreateChat}
            disabled={disabled || isCreating}
          >
            <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
            {isCreating ? "Creating..." : "Create New Chat"}
          </Button>
        </div>
        <ScrollArea className="flex-1">
          <div className="px-3 py-3">
            {chats.length === 0 ? (
              <div className="rounded-xl border border-dashed border-slate-200 px-4 py-6 text-center text-sm leading-6 text-slate-500">
                No chats yet.
              </div>
            ) : (
              <div className="space-y-2">
                {chats.map((chat) => (
                  <div
                    key={chat.id}
                    className={cn(
                      "flex items-start gap-2 rounded-xl border px-4 py-3 transition-colors",
                      selectedThreadId === chat.id
                        ? "border-slate-900 bg-slate-900 text-white"
                        : "border-slate-200 bg-white text-slate-900 hover:bg-slate-50",
                    )}
                  >
                    <button
                      type="button"
                      className="flex min-w-0 flex-1 items-start gap-3 text-left"
                      onClick={() => onSelectChat(chat.id)}
                    >
                      <span
                        className={cn(
                          "mt-0.5 flex size-8 shrink-0 items-center justify-center rounded-lg",
                          selectedThreadId === chat.id
                            ? "bg-white/10 text-white"
                            : "bg-slate-100 text-slate-700",
                        )}
                      >
                        <HugeiconsIcon icon={Chatting01Icon} strokeWidth={2} />
                      </span>
                      <span className="min-w-0 flex-1">
                        <span className="block truncate text-sm font-medium">
                          {chat.title}
                        </span>
                        <span
                          className={cn(
                            "mt-1 block text-xs",
                            selectedThreadId === chat.id
                              ? "text-slate-300"
                              : "text-slate-500",
                          )}
                        >
                          {new Date(chat.updatedAt).toLocaleString()}
                        </span>
                      </span>
                    </button>

                    <Popover
                      open={confirmingThreadId === chat.id}
                      onOpenChange={(open) => {
                        setConfirmingThreadId(open ? chat.id : null);
                      }}
                    >
                      <PopoverTrigger asChild>
                        <Button
                          type="button"
                          variant="ghost"
                          size="icon-sm"
                          className={cn(
                            "mt-0.5 shrink-0 rounded-lg",
                            selectedThreadId === chat.id
                              ? "text-slate-300 hover:bg-white/10 hover:text-white"
                              : "text-slate-500 hover:bg-slate-100 hover:text-slate-950",
                          )}
                          aria-label="Delete chat"
                          disabled={deletingThreadId === chat.id}
                        >
                          <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
                        </Button>
                      </PopoverTrigger>
                      <PopoverContent
                        align="end"
                        className="w-72 space-y-3"
                        onClick={(event) => event.stopPropagation()}
                      >
                        <div className="space-y-1">
                          <p className="text-sm font-semibold text-slate-950">
                            Delete chat?
                          </p>
                          <p className="text-sm leading-6 text-slate-500">
                            This permanently deletes the thread and every message in it.
                          </p>
                        </div>
                        <div className="flex justify-end gap-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => setConfirmingThreadId(null)}
                          >
                            Cancel
                          </Button>
                          <Button
                            type="button"
                            variant="destructive"
                            size="sm"
                            disabled={deletingThreadId === chat.id}
                            onClick={() => {
                              onDeleteChat(chat.id);
                              setConfirmingThreadId(null);
                            }}
                          >
                            {deletingThreadId === chat.id ? "Deleting..." : "Delete"}
                          </Button>
                        </div>
                      </PopoverContent>
                    </Popover>
                  </div>
                ))}
              </div>
            )}
          </div>
        </ScrollArea>
      </DrawerContent>
    </Drawer>
  );
}
