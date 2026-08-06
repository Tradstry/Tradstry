import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type FormEvent,
} from "react";
import {
  CaretDownIcon,
  CaretUpIcon,
  CheckIcon,
  LockSimpleIcon,
  PencilSimpleIcon,
  PlusIcon,
  TagIcon,
  TrashIcon,
} from "@phosphor-icons/react";
import { AnimatePresence, motion, useReducedMotion } from "motion/react";
import { Modal, notify, ScrollArea, Select } from "../user-interface";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Separator } from "@/components/ui/separator";
import { cn } from "@/lib/utils";
import {
  createTag,
  createTagCategory,
  deleteTag,
  deleteTagCategory,
  mergeTags,
  renameTag,
  renameTagCategory,
  reorderTagCategories,
  setTagCategoryColor,
  setTagColor,
  tagCategories as fetchTagCategories,
  tags as fetchTags,
  type Tag,
  type TagCategory,
} from "../../backend";

const DEFAULT_COLOR = "#6b7280";

// --- Color chip -----------------------------------------------------------

/** A solid, obviously-clickable color chip that opens the native picker. */
function ColorSwatch({
  color,
  onChange,
  size = "md",
}: {
  color: string | null | undefined;
  onChange: (c: string) => void;
  size?: "sm" | "md";
}) {
  const c = color ?? DEFAULT_COLOR;
  const ref = useRef<HTMLInputElement>(null);
  return (
    <span className="relative inline-flex shrink-0">
      <button
        type="button"
        title="Change color"
        aria-label="Change color"
        onClick={(e) => {
          e.stopPropagation();
          ref.current?.click();
        }}
        className={cn(
          "cursor-pointer rounded-md shadow-sm ring-1 ring-inset ring-black/10 transition hover:ring-2 hover:ring-ring/60 dark:ring-white/20",
          size === "sm" ? "size-4" : "size-5",
        )}
        style={{ backgroundColor: c }}
      />
      <input
        ref={ref}
        type="color"
        value={c}
        onChange={(e) => onChange(e.target.value)}
        onClick={(e) => e.stopPropagation()}
        tabIndex={-1}
        aria-hidden="true"
        className="pointer-events-none absolute size-0 opacity-0"
      />
    </span>
  );
}

// --- Inline editable name -------------------------------------------------

function useInlineEdit(initial: string, onSave: (v: string) => void) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(initial);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (editing) {
      setDraft(initial);
      inputRef.current?.focus();
      inputRef.current?.select();
    }
  }, [editing, initial]);

  function commit() {
    const trimmed = draft.trim();
    if (trimmed && trimmed !== initial) onSave(trimmed);
    setEditing(false);
  }
  function cancel() {
    setDraft(initial);
    setEditing(false);
  }
  return { editing, setEditing, draft, setDraft, commit, cancel, inputRef };
}

// --- Confirm destructive action -------------------------------------------

function ConfirmDialog({
  open,
  onOpenChange,
  title,
  description,
  confirmLabel,
  isPending,
  onConfirm,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  title: string;
  description: string;
  confirmLabel: string;
  isPending: boolean;
  onConfirm: () => void;
}) {
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

const iconBtn =
  "opacity-0 transition-opacity group-hover:opacity-100 focus-visible:opacity-100";

// --- Tag row --------------------------------------------------------------

function TagRow({
  tag,
  categoryColor,
  allTagsInCategory,
  onChanged,
}: {
  tag: Tag;
  categoryColor: string | null;
  allTagsInCategory: Tag[];
  onChanged: () => void;
}) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [mergeOpen, setMergeOpen] = useState(false);
  const [mergeTargetId, setMergeTargetId] = useState("");
  const [busy, setBusy] = useState(false);

  const edit = useInlineEdit(tag.name, (name) => {
    renameTag(tag.id, name)
      .then(() => {
        notify.success("Tag renamed.");
        onChanged();
      })
      .catch(() => notify.error("Failed to rename tag."));
  });

  const effectiveColor = tag.color ?? categoryColor ?? DEFAULT_COLOR;
  const mergeTargets = allTagsInCategory.filter((t) => t.id !== tag.id);

  async function handleDelete() {
    setBusy(true);
    try {
      await deleteTag(tag.id);
      notify.success("Tag deleted.");
      setConfirmOpen(false);
      onChanged();
    } catch {
      notify.error("Failed to delete tag.");
    } finally {
      setBusy(false);
    }
  }
  async function handleMerge() {
    if (!mergeTargetId) return;
    setBusy(true);
    try {
      await mergeTags(tag.id, mergeTargetId);
      notify.success("Tags merged.");
      setMergeOpen(false);
      onChanged();
    } catch {
      notify.error("Failed to merge tags.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="group flex items-center gap-2.5 rounded-lg px-2.5 py-2 transition-colors hover:bg-muted/60">
      <ColorSwatch
        color={effectiveColor}
        onChange={(color) =>
          setTagColor(tag.id, color)
            .then(onChanged)
            .catch(() => notify.error("Failed to update tag color."))
        }
      />

      {edit.editing ? (
        <>
          <Input
            ref={edit.inputRef}
            value={edit.draft}
            onChange={(e) => edit.setDraft(e.target.value)}
            className="h-7 flex-1"
            onKeyDown={(e) => {
              if (e.key === "Enter") edit.commit();
              if (e.key === "Escape") edit.cancel();
            }}
            onBlur={edit.commit}
          />
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            onClick={edit.commit}
            aria-label="Save"
          >
            <CheckIcon size={14} />
          </Button>
        </>
      ) : (
        <>
          <span className="flex-1 truncate text-sm">{tag.name}</span>
          <div className="flex items-center gap-0.5">
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              aria-label={`Rename tag ${tag.name}`}
              className={iconBtn}
              onClick={() => edit.setEditing(true)}
            >
              <PencilSimpleIcon size={14} />
            </Button>
            {mergeTargets.length > 0 && (
              <Button
                type="button"
                size="xs"
                variant="ghost"
                className={cn(iconBtn, "text-muted-foreground")}
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
              className={cn(
                iconBtn,
                "hover:bg-destructive/10 hover:text-destructive",
              )}
              aria-label={`Delete tag ${tag.name}`}
              onClick={() => setConfirmOpen(true)}
            >
              <TrashIcon size={14} />
            </Button>
          </div>
        </>
      )}

      <ConfirmDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        title={`Delete tag "${tag.name}"?`}
        description="This permanently removes the tag and unassigns it from all trades. This cannot be undone."
        confirmLabel="Delete Tag"
        isPending={busy}
        onConfirm={handleDelete}
      />

      <Dialog open={mergeOpen} onOpenChange={setMergeOpen}>
        <DialogContent className="sm:max-w-sm">
          <DialogHeader>
            <DialogTitle>Merge “{tag.name}”</DialogTitle>
            <DialogDescription>
              All trades tagged “{tag.name}” move to the target tag, and “
              {tag.name}” is deleted. This cannot be undone.
            </DialogDescription>
          </DialogHeader>
          <Select
            label="Merge into"
            value={mergeTargetId}
            onChange={setMergeTargetId}
            options={mergeTargets.map((t) => ({ id: t.id, label: t.name }))}
          />
          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              onClick={() => setMergeOpen(false)}
              disabled={busy}
            >
              Cancel
            </Button>
            <Button
              type="button"
              variant="destructive"
              onClick={handleMerge}
              disabled={busy || !mergeTargetId}
            >
              {busy ? "Merging…" : "Merge Tags"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}

// --- Tags panel (right column) --------------------------------------------

function TagsPanel({
  category,
  tags,
  onChanged,
}: {
  category: TagCategory;
  tags: Tag[];
  onChanged: () => void;
}) {
  const reduce = useReducedMotion();
  const [newTagName, setNewTagName] = useState("");
  const [newTagColor, setNewTagColor] = useState(category.color ?? DEFAULT_COLOR);
  const [busy, setBusy] = useState(false);
  const newTagRef = useRef<HTMLInputElement>(null);

  async function handleCreateTag(e: FormEvent) {
    e.preventDefault();
    const name = newTagName.trim();
    if (!name || busy) return;
    setBusy(true);
    try {
      await createTag(category.id, name, newTagColor);
      notify.success("Tag created.");
      setNewTagName("");
      onChanged();
    } catch {
      notify.error("Failed to create tag.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-1 flex-col gap-3 overflow-hidden">
      {/* Panel header */}
      <div className="flex items-center gap-2.5">
        <ColorSwatch
          color={category.color}
          onChange={(color) =>
            setTagCategoryColor(category.id, color)
              .then(onChanged)
              .catch(() => notify.error("Failed to update category color."))
          }
        />
        <h3 className="text-sm font-semibold">{category.name}</h3>
        <span className="text-xs tabular-nums text-muted-foreground">
          {tags.length} {tags.length === 1 ? "tag" : "tags"}
        </span>
        {category.role !== null && (
          <LockSimpleIcon
            size={13}
            weight="fill"
            className="text-muted-foreground/60"
          >
            <title>Default category</title>
          </LockSimpleIcon>
        )}
      </div>

      {/* Tag list or empty state */}
      {tags.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 rounded-lg border border-dashed p-8 text-center">
          <span className="flex size-11 items-center justify-center rounded-full bg-muted text-muted-foreground">
            <TagIcon size={20} />
          </span>
          <div>
            <p className="text-sm font-medium">
              No tags in {category.name} yet
            </p>
            <p className="mt-1 text-xs text-muted-foreground">
              Create tags to label your trades with this {category.name}.
            </p>
          </div>
          <Button
            type="button"
            size="sm"
            variant="outline"
            onClick={() => newTagRef.current?.focus()}
          >
            <PlusIcon size={14} />
            Add a tag
          </Button>
        </div>
      ) : (
        <ScrollArea className="min-h-0 flex-1 rounded-lg border">
          <div className="p-1.5">
            <AnimatePresence initial={false}>
              {tags.map((tag) => (
                <motion.div
                  key={tag.id}
                  layout={!reduce}
                  initial={reduce ? { opacity: 0 } : { opacity: 0, y: -4 }}
                  animate={{ opacity: 1, y: 0 }}
                  exit={{ opacity: 0, transition: { duration: 0.12 } }}
                  transition={{ duration: 0.16, ease: "easeOut" }}
                >
                  <TagRow
                    tag={tag}
                    categoryColor={category.color}
                    allTagsInCategory={tags}
                    onChanged={onChanged}
                  />
                </motion.div>
              ))}
            </AnimatePresence>
          </div>
        </ScrollArea>
      )}

      {/* Create tag */}
      <form
        onSubmit={handleCreateTag}
        className="flex items-center gap-2 rounded-lg border bg-muted/30 p-2"
      >
        <ColorSwatch color={newTagColor} onChange={setNewTagColor} />
        <Input
          ref={newTagRef}
          placeholder={`New ${category.name} tag…`}
          value={newTagName}
          onChange={(e) => setNewTagName(e.target.value)}
          className="h-8 flex-1 border-0 bg-transparent px-1 shadow-none focus-visible:ring-0"
          disabled={busy}
        />
        <Button
          type="submit"
          size="sm"
          disabled={busy || !newTagName.trim()}
        >
          <PlusIcon size={14} />
          Add
        </Button>
      </form>
    </div>
  );
}

// --- Category row (left column) -------------------------------------------

function CategoryRow({
  category,
  count,
  isSelected,
  onSelect,
  total,
  index,
  onMoveUp,
  onMoveDown,
  onChanged,
}: {
  category: TagCategory;
  count: number;
  isSelected: boolean;
  onSelect: () => void;
  total: number;
  index: number;
  onMoveUp: () => void;
  onMoveDown: () => void;
  onChanged: () => void;
}) {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const isDefault = category.role !== null;

  const edit = useInlineEdit(category.name, (name) => {
    renameTagCategory(category.id, name)
      .then(() => {
        notify.success("Category renamed.");
        onChanged();
      })
      .catch(() => notify.error("Failed to rename category."));
  });

  async function handleDelete() {
    setBusy(true);
    try {
      await deleteTagCategory(category.id);
      notify.success("Category deleted.");
      setConfirmOpen(false);
      onChanged();
    } catch {
      notify.error("Failed to delete category.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      role="button"
      tabIndex={0}
      aria-pressed={isSelected}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      className={cn(
        "group relative flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-2 outline-none transition-colors focus-visible:ring-2 focus-visible:ring-ring/40",
        isSelected ? "bg-primary/10" : "hover:bg-muted/60",
      )}
    >
      {isSelected && (
        <span className="absolute inset-y-2 left-0 w-0.5 rounded-full bg-primary" />
      )}
      <ColorSwatch
        color={category.color}
        onChange={(color) =>
          setTagCategoryColor(category.id, color)
            .then(onChanged)
            .catch(() => notify.error("Failed to update category color."))
        }
      />

      {edit.editing ? (
        <>
          <Input
            ref={edit.inputRef}
            value={edit.draft}
            onChange={(e) => edit.setDraft(e.target.value)}
            className="h-7 flex-1"
            onClick={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              e.stopPropagation();
              if (e.key === "Enter") edit.commit();
              if (e.key === "Escape") edit.cancel();
            }}
            onBlur={edit.commit}
          />
          <Button
            type="button"
            size="icon-xs"
            variant="ghost"
            onClick={(e) => {
              e.stopPropagation();
              edit.commit();
            }}
            aria-label="Save"
          >
            <CheckIcon size={14} />
          </Button>
        </>
      ) : (
        <>
          <span
            className={cn(
              "flex-1 truncate text-sm",
              isSelected ? "font-medium" : "",
            )}
          >
            {category.name}
          </span>

          {/* Resting state: count + default lock */}
          <div className="flex items-center gap-1.5 group-hover:hidden">
            {isDefault && (
              <LockSimpleIcon
                size={12}
                weight="fill"
                className="text-muted-foreground/50"
              >
                <title>Default category</title>
              </LockSimpleIcon>
            )}
            <span className="min-w-4 text-right text-xs tabular-nums text-muted-foreground">
              {count}
            </span>
          </div>

          {/* Hover state: actions */}
          <div className="hidden items-center gap-0.5 group-hover:flex">
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              aria-label="Move up"
              disabled={index === 0}
              onClick={(e) => {
                e.stopPropagation();
                onMoveUp();
              }}
            >
              <CaretUpIcon size={14} />
            </Button>
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              aria-label="Move down"
              disabled={index === total - 1}
              onClick={(e) => {
                e.stopPropagation();
                onMoveDown();
              }}
            >
              <CaretDownIcon size={14} />
            </Button>
            <Button
              type="button"
              size="icon-xs"
              variant="ghost"
              aria-label={`Rename category ${category.name}`}
              onClick={(e) => {
                e.stopPropagation();
                edit.setEditing(true);
              }}
            >
              <PencilSimpleIcon size={14} />
            </Button>
            {isDefault ? (
              <span
                className="flex size-6 items-center justify-center"
                title="Default category — can't be deleted"
              >
                <LockSimpleIcon
                  size={13}
                  weight="fill"
                  className="text-muted-foreground/50"
                />
              </span>
            ) : (
              <Button
                type="button"
                size="icon-xs"
                variant="ghost"
                className="hover:bg-destructive/10 hover:text-destructive"
                aria-label={`Delete category ${category.name}`}
                onClick={(e) => {
                  e.stopPropagation();
                  setConfirmOpen(true);
                }}
              >
                <TrashIcon size={14} />
              </Button>
            )}
          </div>
        </>
      )}

      {!isDefault && (
        <ConfirmDialog
          open={confirmOpen}
          onOpenChange={setConfirmOpen}
          title={`Delete category "${category.name}"?`}
          description="This permanently deletes the category and every tag in it, and unassigns those tags from all trades. This cannot be undone."
          confirmLabel="Delete Category"
          isPending={busy}
          onConfirm={handleDelete}
        />
      )}
    </div>
  );
}

// --- Manager content (two panels) -----------------------------------------

function TagManagerContent({ onExternalChange }: { onExternalChange?: () => void }) {
  const [categories, setCategories] = useState<TagCategory[]>([]);
  const [allTags, setAllTags] = useState<Tag[]>([]);
  const [selectedCategoryId, setSelectedCategoryId] = useState<string | null>(
    null,
  );
  const [loading, setLoading] = useState(true);
  const [newCatName, setNewCatName] = useState("");
  const [newCatColor, setNewCatColor] = useState(DEFAULT_COLOR);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    const [cats, ts] = await Promise.all([fetchTagCategories(), fetchTags()]);
    setCategories(cats);
    setAllTags(ts);
    setLoading(false);
  }, []);

  useEffect(() => {
    load().catch((e) => {
      notify.error(String(e));
      setLoading(false);
    });
  }, [load]);

  const refresh = useCallback(() => {
    load().catch((e) => notify.error(String(e)));
    onExternalChange?.();
  }, [load, onExternalChange]);

  const sortedCats = useMemo(
    () => [...categories].sort((a, b) => a.sortOrder - b.sortOrder),
    [categories],
  );

  const countByCat = useMemo(() => {
    const m = new Map<string, number>();
    for (const t of allTags) {
      if (t.categoryId) m.set(t.categoryId, (m.get(t.categoryId) ?? 0) + 1);
    }
    return m;
  }, [allTags]);

  useEffect(() => {
    if (selectedCategoryId === null && sortedCats[0]) {
      setSelectedCategoryId(sortedCats[0].id);
    }
  }, [sortedCats, selectedCategoryId]);

  const selectedCategory =
    sortedCats.find((c) => c.id === selectedCategoryId) ?? null;

  async function handleMove(index: number, direction: "up" | "down") {
    const list = [...sortedCats];
    const swap = direction === "up" ? index - 1 : index + 1;
    if (swap < 0 || swap >= list.length) return;
    const a = list[index];
    const b = list[swap];
    if (!a || !b) return;
    list[index] = b;
    list[swap] = a;
    try {
      await reorderTagCategories(list.map((c, i) => ({ id: c.id, sortOrder: i })));
      refresh();
    } catch {
      notify.error("Failed to reorder categories.");
    }
  }

  async function handleCreateCategory(e: FormEvent) {
    e.preventDefault();
    const name = newCatName.trim();
    if (!name || busy) return;
    setBusy(true);
    try {
      const created = await createTagCategory(name, newCatColor);
      setNewCatName("");
      setSelectedCategoryId(created.id);
      notify.success("Category created.");
      refresh();
    } catch {
      notify.error("Failed to create category.");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex h-[500px] gap-4 overflow-hidden">
      {/* Left: categories */}
      <div className="flex w-60 shrink-0 flex-col gap-2">
        <p className="px-1 text-[0.7rem] font-semibold uppercase tracking-wider text-muted-foreground">
          Categories
        </p>
        <ScrollArea className="min-h-0 flex-1 rounded-lg border">
          <div className="p-1.5">
            {loading ? (
              <p className="py-6 text-center text-xs text-muted-foreground">
                Loading…
              </p>
            ) : sortedCats.length === 0 ? (
              <p className="py-6 text-center text-xs text-muted-foreground">
                No categories yet.
              </p>
            ) : (
              sortedCats.map((cat, index) => (
                <CategoryRow
                  key={cat.id}
                  category={cat}
                  count={countByCat.get(cat.id) ?? 0}
                  isSelected={cat.id === selectedCategoryId}
                  onSelect={() => setSelectedCategoryId(cat.id)}
                  total={sortedCats.length}
                  index={index}
                  onMoveUp={() => handleMove(index, "up")}
                  onMoveDown={() => handleMove(index, "down")}
                  onChanged={refresh}
                />
              ))
            )}
          </div>
        </ScrollArea>

        <form
          onSubmit={handleCreateCategory}
          className="flex items-center gap-2 rounded-lg border bg-muted/30 p-2"
        >
          <ColorSwatch color={newCatColor} onChange={setNewCatColor} />
          <Input
            placeholder="New category…"
            value={newCatName}
            onChange={(e) => setNewCatName(e.target.value)}
            className="h-8 flex-1 border-0 bg-transparent px-1 shadow-none focus-visible:ring-0"
            disabled={busy}
          />
          <Button
            type="submit"
            size="icon-sm"
            disabled={busy || !newCatName.trim()}
            aria-label="Add category"
          >
            <PlusIcon size={14} />
          </Button>
        </form>
      </div>

      <Separator orientation="vertical" />

      {/* Right: tags in selected category */}
      <div className="flex flex-1 flex-col overflow-hidden">
        {selectedCategory ? (
          <TagsPanel
            key={selectedCategory.id}
            category={selectedCategory}
            tags={allTags.filter((t) => t.categoryId === selectedCategory.id)}
            onChanged={refresh}
          />
        ) : (
          <div className="flex flex-1 items-center justify-center text-sm text-muted-foreground">
            Select a category to manage its tags.
          </div>
        )}
      </div>
    </div>
  );
}

// --- Public component ------------------------------------------------------

export function TagManager({ onChanged }: { onChanged?: () => void }) {
  return (
    <Modal
      trigger={
        <Button type="button" variant="outline" size="sm">
          <TagIcon size={14} />
          Manage Tags
        </Button>
      }
      icon={<TagIcon size={16} weight="bold" />}
      title="Manage Tags"
      description="Organize the categories and tags you use to label trades."
      size="xl"
    >
      <Separator />
      <TagManagerContent onExternalChange={onChanged} />
    </Modal>
  );
}
