import { useMemo, useState, type CSSProperties } from "react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import type { Tag, TagCategory } from "../../backend";

/** Fallback color when neither tag nor category has one. */
const DEFAULT_COLOR = "#6b7280";

function colorStyle(color: string | null | undefined): CSSProperties {
  const c = color ?? DEFAULT_COLOR;
  // `22` is a hex alpha for a translucent background.
  return { backgroundColor: `${c}22`, borderColor: c, color: c };
}

type TagPickerProps = {
  /** The category this picker manages. */
  category: TagCategory;
  /** All of the user's tags; the picker shows only this category's. */
  tags: Tag[];
  /** Selected tag ids within this category (controlled). */
  selectedTagIds: string[];
  /** Called with the full new array of selected ids for this category. */
  onChange: (ids: string[]) => void;
  /** Create a tag in this category and return it (parent updates its list). */
  onCreate: (name: string) => Promise<Tag>;
};

/**
 * Multiselect chip/combobox for a single tag category — matches the web's
 * TagPicker: colored chips with remove buttons, a searchable list, and an
 * inline "Create" option when the search doesn't match an existing tag.
 */
export function TagPicker({
  category,
  tags,
  selectedTagIds,
  onChange,
  onCreate,
}: TagPickerProps) {
  const reduce = useReducedMotion();
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");
  const [creating, setCreating] = useState(false);

  const categoryTags = useMemo(
    () => tags.filter((t) => t.categoryId === category.id),
    [tags, category.id],
  );

  const filtered = useMemo(() => {
    const q = search.trim().toLowerCase();
    if (!q) return categoryTags;
    return categoryTags.filter((t) => t.name.toLowerCase().includes(q));
  }, [categoryTags, search]);

  const exactMatch = categoryTags.some(
    (t) => t.name.toLowerCase() === search.trim().toLowerCase(),
  );
  const canCreate = search.trim().length > 0 && !exactMatch;

  const categoryColor = category.color ?? null;
  const selectedTags = categoryTags.filter((t) =>
    selectedTagIds.includes(t.id),
  );

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
    if (!name || creating) return;
    setCreating(true);
    try {
      const created = await onCreate(name);
      onChange([...selectedTagIds, created.id]);
      setSearch("");
    } catch {
      // Creation errors are surfaced by the caller.
    } finally {
      setCreating(false);
    }
  }

  return (
    <div className="flex flex-col gap-1.5">
      {selectedTags.length > 0 && (
        <div className="flex flex-wrap gap-1">
          <AnimatePresence initial={false}>
            {selectedTags.map((tag) => (
              <motion.span
                key={tag.id}
                layout
                className="inline-flex"
                initial={reduce ? { opacity: 0 } : { opacity: 0, scale: 0.7 }}
                animate={{ opacity: 1, scale: 1 }}
                exit={reduce ? { opacity: 0 } : { opacity: 0, scale: 0.7 }}
                transition={{ duration: 0.15, ease: "easeOut" }}
              >
                <Badge
                  variant="outline"
                  className="gap-1 border pr-1 font-normal"
                  style={colorStyle(tag.color ?? categoryColor)}
                >
                  {tag.name}
                  <button
                    type="button"
                    aria-label={`Remove ${tag.name}`}
                    className="ml-0.5 cursor-pointer rounded-full opacity-60 transition-opacity hover:opacity-100"
                    onClick={() => removeTag(tag.id)}
                  >
                    ×
                  </button>
                </Badge>
              </motion.span>
            ))}
          </AnimatePresence>
        </div>
      )}

      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            type="button"
            variant="outline"
            size="sm"
            className="h-8 justify-start bg-muted/50 font-normal text-muted-foreground"
            aria-label={`Add ${category.name} tag`}
          >
            + Add {category.name} tag
          </Button>
        </PopoverTrigger>
        <PopoverContent
          className="w-56 gap-2 p-2"
          align="start"
          onOpenAutoFocus={(e) => e.preventDefault()}
        >
          <input
            type="text"
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Search or create…"
            className="h-7 w-full rounded-md border border-input bg-transparent px-2 text-xs outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30"
            aria-label={`Search ${category.name} tags`}
            onKeyDown={(e) => e.stopPropagation()}
          />

          <div
            className="max-h-48 overflow-y-auto"
            role="listbox"
            aria-multiselectable="true"
            aria-label={`${category.name} tags`}
          >
            {filtered.length === 0 && !canCreate ? (
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
                    <span
                      className="inline-block size-2 shrink-0 rounded-full"
                      style={{ backgroundColor: style.color as string }}
                    />
                    <span className="flex-1 truncate text-left">{tag.name}</span>
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

            {canCreate && (
              <button
                type="button"
                className="mt-1 flex w-full cursor-pointer items-center gap-2 rounded-md border border-dashed border-muted-foreground/40 px-2 py-1 text-xs text-muted-foreground transition-colors hover:border-ring hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                onClick={handleCreate}
                disabled={creating}
              >
                <span>+</span>
                <span>Create “{search.trim()}”</span>
              </button>
            )}
          </div>
        </PopoverContent>
      </Popover>
    </div>
  );
}
