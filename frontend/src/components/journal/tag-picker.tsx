"use client";

import * as React from "react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { ScrollArea } from "@/components/ui/scroll-area";
import { useCreateTag, useTags } from "@/hooks/tags";
import type { TagCategory } from "@/lib/types/tags";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Fallback color when neither tag nor category has a color. */
const DEFAULT_COLOR = "#6b7280";

function colorStyle(color: string | null | undefined): React.CSSProperties {
  const c = color ?? DEFAULT_COLOR;
  return { backgroundColor: `${c}22`, borderColor: c, color: c };
}

// ---------------------------------------------------------------------------
// TagPicker
// ---------------------------------------------------------------------------

export interface TagPickerProps {
  /** The category this picker belongs to. */
  category: TagCategory;
  /** Currently selected tag ids within this category. */
  selectedTagIds: string[];
  /** Called whenever the selection changes. */
  onChange: (ids: string[]) => void;
  /** Optional extra class applied to the root wrapper. */
  className?: string;
}

/**
 * A multiselect chip/combobox for a single tag category.
 *
 * - Selected tags are shown as colored Badge chips with an ×-button.
 * - The popover lists all tags in the category, filtered by a search input.
 * - "Create tag" inline option appears when the search term doesn't match any
 *   existing tag; calls `useCreateTag` and auto-selects the new tag.
 */
export function TagPicker({
  category,
  selectedTagIds,
  onChange,
  className,
}: TagPickerProps) {
  const [open, setOpen] = React.useState(false);
  const [search, setSearch] = React.useState("");
  const tagsQuery = useTags(category.id);
  const createTag = useCreateTag();

  const allTags = tagsQuery.data ?? [];

  const filtered = React.useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return allTags;
    return allTags.filter((t) => t.name.toLowerCase().includes(q));
  }, [allTags, search]);

  const exactMatch = allTags.some(
    (t) => t.name.toLowerCase() === search.trim().toLowerCase(),
  );
  const canCreate = search.trim().length > 0 && !exactMatch;

  function toggleTag(id: string) {
    if (selectedTagIds.includes(id)) {
      onChange(selectedTagIds.filter((x) => x !== id));
    } else {
      onChange([...selectedTagIds, id]);
    }
  }

  function removeTag(id: string) {
    onChange(selectedTagIds.filter((x) => x !== id));
  }

  async function handleCreate() {
    const name = search.trim();
    if (!name) return;
    try {
      const created = await createTag.mutateAsync({
        categoryId: category.id,
        name,
        color: null,
      });
      onChange([...selectedTagIds, created.id]);
      setSearch("");
    } catch {
      // createTag errors are surfaced by the mutation; nothing else to do here.
    }
  }

  const selectedTags = allTags.filter((t) => selectedTagIds.includes(t.id));
  const categoryColor = category.color ?? null;

  return (
    <div className={cn("flex flex-col gap-1.5", className)}>
      {/* Selected tag chips */}
      {selectedTags.length > 0 && (
        <div className="flex flex-wrap gap-1">
          {selectedTags.map((tag) => {
            const style = colorStyle(tag.color ?? categoryColor);
            return (
              <Badge
                key={tag.id}
                variant="outline"
                className="gap-1 border pr-1 font-normal"
                style={style}
              >
                {tag.name}
                <button
                  type="button"
                  aria-label={`Remove ${tag.name}`}
                  className="ml-0.5 cursor-pointer rounded-full opacity-60 transition-opacity hover:opacity-100 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring"
                  onClick={() => removeTag(tag.id)}
                >
                  ×
                </button>
              </Badge>
            );
          })}
        </div>
      )}

      {/* Popover trigger */}
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-7 justify-start text-xs font-normal text-muted-foreground"
            aria-label={`Add ${category.name} tag`}
          >
            + Add {category.name} tag
          </Button>
        </PopoverTrigger>
        <PopoverContent
          className="w-56 p-2"
          align="start"
          onOpenAutoFocus={(e) => e.preventDefault()}
        >
          {/* Search input */}
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder={`Search or create…`}
            className="mb-2 h-7 w-full rounded-md border border-input bg-input/20 px-2 py-0.5 text-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30"
            aria-label={`Search ${category.name} tags`}
            // Allow typing inside the popover without closing it
            onKeyDown={(e) => e.stopPropagation()}
          />

          {/* Tag list */}
          <ScrollArea
            className="[&>[data-radix-scroll-area-viewport]]:max-h-48"
            role="listbox"
            aria-multiselectable="true"
            aria-label={`${category.name} tags`}
          >
            {tagsQuery.isLoading ? (
              <p className="py-2 text-center text-xs text-muted-foreground">
                Loading…
              </p>
            ) : filtered.length === 0 && !canCreate ? (
              <p className="py-2 text-center text-xs text-muted-foreground">
                No tags found.
              </p>
            ) : (
              filtered.map((tag) => {
                const selected = selectedTagIds.includes(tag.id);
                const style = colorStyle(tag.color ?? categoryColor);
                return (
                  <button
                    key={tag.id}
                    type="button"
                    role="option"
                    aria-selected={selected}
                    className={cn(
                      "flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1 text-xs transition-colors hover:bg-muted",
                      selected && "bg-muted/60",
                    )}
                    onClick={() => toggleTag(tag.id)}
                  >
                    {/* Color dot */}
                    <span
                      className="inline-block size-2 shrink-0 rounded-full"
                      style={{ backgroundColor: style.color as string }}
                    />
                    <span className="flex-1 truncate text-left">
                      {tag.name}
                    </span>
                    {selected && (
                      <span
                        className="ml-auto text-[10px] opacity-60"
                        aria-hidden="true"
                      >
                        ✓
                      </span>
                    )}
                  </button>
                );
              })
            )}

            {/* Create new tag option */}
            {canCreate && (
              <button
                type="button"
                className="mt-1 flex w-full cursor-pointer items-center gap-2 rounded-md border border-dashed border-muted-foreground/40 px-2 py-1 text-xs text-muted-foreground transition-colors hover:border-ring hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                onClick={handleCreate}
                disabled={createTag.isPending}
              >
                <span>+</span>
                <span>Create &ldquo;{search.trim()}&rdquo;</span>
              </button>
            )}
          </ScrollArea>
        </PopoverContent>
      </Popover>
    </div>
  );
}
