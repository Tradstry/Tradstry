"use client";

import {
  Add01Icon,
  ArrowDown01Icon,
  ArrowUp01Icon,
  Delete02Icon,
  PencilEdit01Icon,
  Tick02Icon,
} from "@hugeicons/core-free-icons";
import { HugeiconsIcon } from "@hugeicons/react";
import * as React from "react";
import { toast } from "sonner";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Separator } from "@/components/ui/separator";
import {
  useCreateTag,
  useCreateTagCategory,
  useDeleteTag,
  useDeleteTagCategory,
  useMergeTags,
  useRenameTag,
  useRenameTagCategory,
  useReorderTagCategories,
  useSetTagCategoryColor,
  useSetTagColor,
  useTagCategories,
  useTags,
} from "@/hooks/tags";
import type { Tag, TagCategory } from "@/lib/types/tags";
import { cn } from "@/lib/utils";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

const DEFAULT_COLOR = "#6b7280";

function colorDot(color: string | null | undefined, size = "size-3") {
  const c = color ?? DEFAULT_COLOR;
  return (
    <span
      className={cn("inline-block shrink-0 rounded-full border", size)}
      style={{ backgroundColor: `${c}22`, borderColor: c }}
      aria-hidden="true"
    />
  );
}

/** Tiny inline hex color input that looks native to the design system. */
function ColorInput({
  value,
  onChange,
  id,
}: {
  value: string;
  onChange: (v: string) => void;
  id?: string;
}) {
  return (
    <div className="relative flex items-center gap-1.5">
      <span
        className="inline-block size-5 shrink-0 rounded-full border border-input"
        style={{ backgroundColor: value || DEFAULT_COLOR }}
        aria-hidden="true"
      />
      <input
        id={id}
        type="color"
        value={value || DEFAULT_COLOR}
        onChange={(e) => onChange(e.target.value)}
        className="h-6 w-10 cursor-pointer rounded border border-input bg-transparent p-0.5"
        aria-label="Pick color"
      />
    </div>
  );
}

// ---------------------------------------------------------------------------
// Confirm destructive action dialog
// ---------------------------------------------------------------------------

interface ConfirmDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  isPending: boolean;
  onConfirm: () => void;
}

function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  isPending,
  onConfirm,
}: ConfirmDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-sm">
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription>{description}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => onOpenChange(false)}
            disabled={isPending}
          >
            Cancel
          </Button>
          <Button
            type="button"
            variant="destructive"
            onClick={onConfirm}
            disabled={isPending}
          >
            {isPending ? "Working…" : confirmLabel}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

// ---------------------------------------------------------------------------
// Inline editable name — shared hook
// ---------------------------------------------------------------------------

function useInlineEdit(initial: string, onSave: (v: string) => void) {
  const [editing, setEditing] = React.useState(false);
  const [draft, setDraft] = React.useState(initial);
  const inputRef = React.useRef<HTMLInputElement>(null);

  React.useEffect(() => {
    if (editing) {
      setDraft(initial);
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing, initial]);

  function commit() {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== initial) {
      onSave(trimmed);
    }
    setEditing(false);
  }

  function cancel() {
    setDraft(initial);
    setEditing(false);
  }

  return { editing, setEditing, draft, setDraft, commit, cancel, inputRef };
}

// ---------------------------------------------------------------------------
// Tag row inside the tags panel
// ---------------------------------------------------------------------------

interface TagRowProps {
  tag: Tag;
  categoryColor: string | null;
  allTagsInCategory: Tag[];
}

function TagRow({ tag, categoryColor, allTagsInCategory }: TagRowProps) {
  const renameTag = useRenameTag();
  const setTagColor = useSetTagColor();
  const deleteTag = useDeleteTag();
  const mergeTags = useMergeTags();

  const [confirmDeleteOpen, setConfirmDeleteOpen] = React.useState(false);
  const [mergeOpen, setMergeOpen] = React.useState(false);
  const [mergeTargetId, setMergeTargetId] = React.useState<string>("");

  const mergeSelectId = `merge-target-${tag.id}`;

  const { editing, setEditing, draft, setDraft, commit, cancel, inputRef } =
    useInlineEdit(tag.name, (name) => {
      renameTag.mutate(
        { id: tag.id, name },
        {
          onSuccess: () => toast.success("Tag renamed."),
          onError: () => toast.error("Failed to rename tag."),
        },
      );
    });

  const effectiveColor = tag.color ?? categoryColor ?? DEFAULT_COLOR;

  const mergeTargets = allTagsInCategory.filter((t) => t.id !== tag.id);

  function handleDelete() {
    deleteTag.mutate(tag.id, {
      onSuccess: () => {
        toast.success("Tag deleted.");
        setConfirmDeleteOpen(false);
      },
      onError: () => toast.error("Failed to delete tag."),
    });
  }

  function handleMerge() {
    if (!mergeTargetId) return;
    mergeTags.mutate(
      { fromId: tag.id, intoId: mergeTargetId },
      {
        onSuccess: () => {
          toast.success("Tags merged.");
          setMergeOpen(false);
        },
        onError: () => toast.error("Failed to merge tags."),
      },
    );
  }

  return (
    <div className="group flex items-center gap-2 rounded-md px-2 py-1.5 hover:bg-muted/50">
      {/* Color swatch — clicking the visible dot triggers the hidden input */}
      <button
        type="button"
        title="Change color"
        className="shrink-0 cursor-pointer"
        onClick={() => {
          document.getElementById(`tag-color-${tag.id}`)?.click();
        }}
      >
        {colorDot(effectiveColor)}
      </button>
      <input
        id={`tag-color-${tag.id}`}
        type="color"
        className="sr-only"
        value={tag.color ?? categoryColor ?? DEFAULT_COLOR}
        onChange={(e) => {
          setTagColor.mutate(
            { id: tag.id, color: e.target.value },
            { onError: () => toast.error("Failed to update tag color.") },
          );
        }}
        aria-label={`Color for tag ${tag.name}`}
      />

      {/* Name (inline-editable) */}
      {editing ? (
        <>
          <Input
            ref={inputRef}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            className="h-6 flex-1 text-xs"
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") cancel();
            }}
            onBlur={commit}
          />
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            onClick={commit}
            disabled={renameTag.isPending}
            aria-label="Save"
          >
            <HugeiconsIcon icon={Tick02Icon} strokeWidth={2} />
          </Button>
        </>
      ) : (
        <span className="flex-1 truncate text-xs">{tag.name}</span>
      )}

      {/* Actions — visible on hover */}
      {!editing && (
        <>
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label={`Rename tag ${tag.name}`}
            className="opacity-0 transition-opacity group-hover:opacity-100"
            onClick={() => setEditing(true)}
          >
            <HugeiconsIcon icon={PencilEdit01Icon} strokeWidth={2} />
          </Button>

          {/* Merge button (only if there are other tags) */}
          {mergeTargets.length > 0 && (
            <Button
              type="button"
              size="xs"
              variant="ghost"
              className="text-muted-foreground opacity-0 transition-opacity group-hover:opacity-100"
              aria-label={`Merge tag ${tag.name}`}
              onClick={() => {
                setMergeTargetId(mergeTargets[0]?.id ?? "");
                setMergeOpen(true);
              }}
            >
              Merge
            </Button>
          )}

          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            className="opacity-0 transition-opacity group-hover:opacity-100 hover:bg-destructive/10 hover:text-destructive"
            aria-label={`Delete tag ${tag.name}`}
            onClick={() => setConfirmDeleteOpen(true)}
          >
            <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
          </Button>
        </>
      )}

      {/* Delete confirm */}
      <ConfirmDialog
        open={confirmDeleteOpen}
        onOpenChange={setConfirmDeleteOpen}
        title={`Delete tag "${tag.name}"?`}
        description="This will permanently remove the tag and unassign it from all trades. This action cannot be undone."
        confirmLabel="Delete Tag"
        isPending={deleteTag.isPending}
        onConfirm={handleDelete}
      />

      {/* Merge dialog */}
      <Dialog open={mergeOpen} onOpenChange={setMergeOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Merge &ldquo;{tag.name}&rdquo;</DialogTitle>
            <DialogDescription>
              All trades tagged with &ldquo;{tag.name}&rdquo; will be re-pointed
              to the target tag, and &ldquo;{tag.name}&rdquo; will be deleted.
              This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <div className="flex flex-col gap-2">
            <label htmlFor={mergeSelectId} className="text-xs font-medium">
              Merge into:
            </label>
            <select
              id={mergeSelectId}
              className="h-7 w-full rounded-md border border-input bg-input/20 px-2 text-xs outline-none focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/30"
              value={mergeTargetId}
              onChange={(e) => setMergeTargetId(e.target.value)}
            >
              {mergeTargets.map((t) => (
                <option key={t.id} value={t.id}>
                  {t.name}
                </option>
              ))}
            </select>
          </div>
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setMergeOpen(false)}
              disabled={mergeTags.isPending}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={handleMerge}
              disabled={mergeTags.isPending || !mergeTargetId}
            >
              {mergeTags.isPending ? "Merging…" : "Merge Tags"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Tags panel (right side when a category is selected)
// ---------------------------------------------------------------------------

interface TagsPanelProps {
  category: TagCategory;
}

function TagsPanel({ category }: TagsPanelProps) {
  const tagsQuery = useTags(category.id);
  const createTag = useCreateTag();

  const [newTagName, setNewTagName] = React.useState("");
  const [newTagColor, setNewTagColor] = React.useState(
    category.color ?? DEFAULT_COLOR,
  );

  const tags = tagsQuery.data ?? [];

  function handleCreateTag(e: React.FormEvent) {
    e.preventDefault();
    const name = newTagName.trim();
    if (!name) return;
    createTag.mutate(
      { categoryId: category.id, name, color: newTagColor },
      {
        onSuccess: () => {
          toast.success("Tag created.");
          setNewTagName("");
        },
        onError: () => toast.error("Failed to create tag."),
      },
    );
  }

  return (
    <div className="flex flex-1 flex-col gap-3 overflow-hidden">
      <div className="flex items-center gap-2">
        {colorDot(category.color)}
        <h3 className="text-sm font-medium">{category.name}</h3>
        {category.role !== null && (
          <Badge variant="secondary" className="text-[0.6rem]">
            {category.role}
          </Badge>
        )}
      </div>

      {/* Tag list */}
      <ScrollArea className="flex-1 rounded-md border">
        <div className="p-1">
          {tagsQuery.isLoading ? (
            <p className="py-4 text-center text-xs text-muted-foreground">
              Loading tags…
            </p>
          ) : tags.length === 0 ? (
            <p className="py-4 text-center text-xs text-muted-foreground">
              No tags yet. Create one below.
            </p>
          ) : (
            tags.map((tag) => (
              <TagRow
                key={tag.id}
                tag={tag}
                categoryColor={category.color}
                allTagsInCategory={tags}
              />
            ))
          )}
        </div>
      </ScrollArea>

      {/* Create new tag */}
      <form
        onSubmit={handleCreateTag}
        className="flex items-center gap-2 rounded-md border border-dashed p-2"
      >
        <ColorInput
          value={newTagColor}
          onChange={setNewTagColor}
          id={`new-tag-color-${category.id}`}
        />
        <Input
          placeholder="New tag name…"
          value={newTagName}
          onChange={(e) => setNewTagName(e.target.value)}
          className="flex-1 text-xs"
          disabled={createTag.isPending}
        />
        <Button
          type="submit"
          size="icon-sm"
          variant="outline"
          disabled={createTag.isPending || !newTagName.trim()}
          aria-label="Create tag"
        >
          <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
        </Button>
      </form>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Category row in the left panel
// ---------------------------------------------------------------------------

interface CategoryRowProps {
  category: TagCategory;
  isSelected: boolean;
  onSelect: () => void;
  totalCategories: number;
  index: number;
  onMoveUp: () => void;
  onMoveDown: () => void;
}

function CategoryRow({
  category,
  isSelected,
  onSelect,
  totalCategories,
  index,
  onMoveUp,
  onMoveDown,
}: CategoryRowProps) {
  const renameCategory = useRenameTagCategory();
  const setCategoryColor = useSetTagCategoryColor();
  const deleteCategory = useDeleteTagCategory();

  const [confirmDeleteOpen, setConfirmDeleteOpen] = React.useState(false);

  const isDeletable = category.role === null;

  const { editing, setEditing, draft, setDraft, commit, cancel, inputRef } =
    useInlineEdit(category.name, (name) => {
      renameCategory.mutate(
        { id: category.id, name },
        {
          onSuccess: () => toast.success("Category renamed."),
          onError: () => toast.error("Failed to rename category."),
        },
      );
    });

  function handleDelete() {
    deleteCategory.mutate(category.id, {
      onSuccess: () => {
        toast.success("Category deleted.");
        setConfirmDeleteOpen(false);
      },
      onError: () => toast.error("Failed to delete category."),
    });
  }

  return (
    <button
      type="button"
      className={cn(
        "group flex w-full cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-left transition-colors",
        isSelected ? "bg-muted" : "hover:bg-muted/50",
      )}
      onClick={onSelect}
      aria-pressed={isSelected}
      aria-label={`Select category ${category.name}`}
    >
      {/* Hidden color input triggered by dot click */}
      <button
        type="button"
        className="shrink-0 cursor-pointer"
        title="Change category color"
        onClick={(e) => {
          e.stopPropagation();
          document.getElementById(`cat-color-${category.id}`)?.click();
        }}
        aria-label={`Change color for category ${category.name}`}
      >
        {colorDot(category.color)}
      </button>
      <input
        id={`cat-color-${category.id}`}
        type="color"
        className="sr-only"
        value={category.color ?? DEFAULT_COLOR}
        onChange={(e) => {
          setCategoryColor.mutate(
            { id: category.id, color: e.target.value },
            { onError: () => toast.error("Failed to update category color.") },
          );
        }}
        aria-label={`Color for category ${category.name}`}
        onClick={(e) => e.stopPropagation()}
      />

      {/* Name */}
      {editing ? (
        <>
          <Input
            ref={inputRef}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            className="h-6 flex-1 text-xs"
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") commit();
              if (e.key === "Escape") cancel();
            }}
            onBlur={commit}
            onClick={(e) => e.stopPropagation()}
          />
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            onClick={(e) => {
              e.stopPropagation();
              commit();
            }}
            aria-label="Save"
          >
            <HugeiconsIcon icon={Tick02Icon} strokeWidth={2} />
          </Button>
        </>
      ) : (
        <span className="flex-1 truncate text-xs">{category.name}</span>
      )}

      {/* Role badge */}
      {category.role !== null && (
        <Badge variant="secondary" className="shrink-0 text-[0.6rem]">
          {category.role}
        </Badge>
      )}

      {/* Reorder + edit + delete — visible on hover */}
      {!editing && (
        <>
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label="Move up"
            className="opacity-0 transition-opacity group-hover:opacity-100"
            disabled={index === 0}
            onClick={(e) => {
              e.stopPropagation();
              onMoveUp();
            }}
          >
            <HugeiconsIcon icon={ArrowUp01Icon} strokeWidth={2} />
          </Button>
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label="Move down"
            className="opacity-0 transition-opacity group-hover:opacity-100"
            disabled={index === totalCategories - 1}
            onClick={(e) => {
              e.stopPropagation();
              onMoveDown();
            }}
          >
            <HugeiconsIcon icon={ArrowDown01Icon} strokeWidth={2} />
          </Button>
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            aria-label={`Rename category ${category.name}`}
            className="opacity-0 transition-opacity group-hover:opacity-100"
            onClick={(e) => {
              e.stopPropagation();
              setEditing(true);
            }}
          >
            <HugeiconsIcon icon={PencilEdit01Icon} strokeWidth={2} />
          </Button>
          {isDeletable ? (
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              className="opacity-0 transition-opacity group-hover:opacity-100 hover:bg-destructive/10 hover:text-destructive"
              aria-label={`Delete category ${category.name}`}
              onClick={(e) => {
                e.stopPropagation();
                setConfirmDeleteOpen(true);
              }}
            >
              <HugeiconsIcon icon={Delete02Icon} strokeWidth={2} />
            </Button>
          ) : (
            /* Placeholder to keep layout stable for roled categories */
            <span className="inline-block size-5" aria-hidden="true" />
          )}
        </>
      )}

      {/* Delete confirm */}
      {isDeletable && (
        <ConfirmDialog
          open={confirmDeleteOpen}
          onOpenChange={setConfirmDeleteOpen}
          title={`Delete category "${category.name}"?`}
          description="This will permanently delete the category and all tags within it. All trades tagged with those tags will be unassigned. This action cannot be undone."
          confirmLabel="Delete Category"
          isPending={deleteCategory.isPending}
          onConfirm={handleDelete}
        />
      )}
    </button>
  );
}

// ---------------------------------------------------------------------------
// Main tag-manager content (rendered inside the outer Dialog)
// ---------------------------------------------------------------------------

function TagManagerContent() {
  const categoriesQuery = useTagCategories();
  const createCategory = useCreateTagCategory();
  const reorderCategories = useReorderTagCategories();

  const [selectedCategoryId, setSelectedCategoryId] = React.useState<
    string | null
  >(null);

  const [newCatName, setNewCatName] = React.useState("");
  const [newCatColor, setNewCatColor] = React.useState(DEFAULT_COLOR);

  const categories = React.useMemo(
    () =>
      [...(categoriesQuery.data ?? [])].sort(
        (a, b) => a.sortOrder - b.sortOrder,
      ),
    [categoriesQuery.data],
  );

  // Auto-select first category if none selected
  React.useEffect(() => {
    if (
      selectedCategoryId === null &&
      categories.length > 0 &&
      categories[0] !== undefined
    ) {
      setSelectedCategoryId(categories[0].id);
    }
  }, [categories, selectedCategoryId]);

  const selectedCategory =
    categories.find((c) => c.id === selectedCategoryId) ?? null;

  function handleMoveCategory(index: number, direction: "up" | "down") {
    const newList = [...categories];
    const swapIndex = direction === "up" ? index - 1 : index + 1;
    if (swapIndex < 0 || swapIndex >= newList.length) return;
    const temp = newList[index];
    if (!temp) return;
    const swapItem = newList[swapIndex];
    if (!swapItem) return;
    newList[index] = swapItem;
    newList[swapIndex] = temp;
    reorderCategories.mutate(
      newList.map((cat, i) => ({ id: cat.id, sortOrder: i })),
      { onError: () => toast.error("Failed to reorder categories.") },
    );
  }

  function handleCreateCategory(e: React.FormEvent) {
    e.preventDefault();
    const name = newCatName.trim();
    if (!name) return;
    createCategory.mutate(
      { name, color: newCatColor },
      {
        onSuccess: (created) => {
          toast.success("Category created.");
          setNewCatName("");
          setSelectedCategoryId(created.id);
        },
        onError: () => toast.error("Failed to create category."),
      },
    );
  }

  return (
    <div className="flex h-[520px] gap-0 overflow-hidden">
      {/* ── Left column: categories ─────────────────────────────── */}
      <div className="flex w-52 shrink-0 flex-col gap-2 border-r pr-3">
        <p className="text-xs font-semibold uppercase tracking-wide text-muted-foreground">
          Categories
        </p>

        <ScrollArea className="flex-1 rounded-md border">
          <div className="p-1">
            {categoriesQuery.isLoading ? (
              <p className="py-4 text-center text-xs text-muted-foreground">
                Loading…
              </p>
            ) : categories.length === 0 ? (
              <p className="py-4 text-center text-xs text-muted-foreground">
                No categories yet.
              </p>
            ) : (
              categories.map((cat, index) => (
                <CategoryRow
                  key={cat.id}
                  category={cat}
                  isSelected={cat.id === selectedCategoryId}
                  onSelect={() => setSelectedCategoryId(cat.id)}
                  totalCategories={categories.length}
                  index={index}
                  onMoveUp={() => handleMoveCategory(index, "up")}
                  onMoveDown={() => handleMoveCategory(index, "down")}
                />
              ))
            )}
          </div>
        </ScrollArea>

        {/* Create new category */}
        <form
          onSubmit={handleCreateCategory}
          className="flex flex-col gap-1.5 rounded-md border border-dashed p-2"
        >
          <p className="text-[0.65rem] font-medium text-muted-foreground">
            New category
          </p>
          <div className="flex items-center gap-1.5">
            <ColorInput
              value={newCatColor}
              onChange={setNewCatColor}
              id="new-category-color"
            />
            <Input
              placeholder="Name…"
              value={newCatName}
              onChange={(e) => setNewCatName(e.target.value)}
              className="flex-1 text-xs"
              disabled={createCategory.isPending}
            />
          </div>
          <Button
            type="submit"
            size="sm"
            variant="outline"
            className="w-full"
            disabled={createCategory.isPending || !newCatName.trim()}
          >
            <HugeiconsIcon icon={Add01Icon} strokeWidth={2} />
            Add category
          </Button>
        </form>
      </div>

      {/* ── Right column: tags in selected category ──────────────── */}
      <div className="flex flex-1 flex-col overflow-hidden pl-3">
        {selectedCategory !== null ? (
          <TagsPanel category={selectedCategory} />
        ) : (
          <div className="flex flex-1 items-center justify-center text-xs text-muted-foreground">
            Select a category to manage its tags.
          </div>
        )}
      </div>
    </div>
  );
}

// ---------------------------------------------------------------------------
// Public component — TagManager (Dialog with trigger button)
// ---------------------------------------------------------------------------

export interface TagManagerProps {
  /** Optional custom trigger. Defaults to an outlined "Manage Tags" button. */
  trigger?: React.ReactNode;
}

export function TagManager({ trigger }: TagManagerProps) {
  const [open, setOpen] = React.useState(false);

  const defaultTrigger = (
    <Button type="button" variant="outline" size="default">
      Manage Tags
    </Button>
  );

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogTrigger asChild>{trigger ?? defaultTrigger}</DialogTrigger>
      <DialogContent
        className="sm:max-w-2xl"
        aria-describedby="tag-manager-description"
      >
        <DialogHeader>
          <DialogTitle>Manage Tags</DialogTitle>
          <DialogDescription id="tag-manager-description">
            Create, rename, recolor, reorder, and delete tag categories and the
            tags within them.
          </DialogDescription>
        </DialogHeader>
        <Separator />
        <TagManagerContent />
      </DialogContent>
    </Dialog>
  );
}
