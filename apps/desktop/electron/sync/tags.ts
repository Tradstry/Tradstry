import type { DesktopDatabase } from "./database.ts";
import { transaction } from "./database.ts";
import { enqueueMutation, uuidV7 } from "./mutations.ts";

export type LocalTag = { id: string; name: string; color: string | null; categoryId: string };
export type LocalTagCategory = {
  id: string;
  name: string;
  role: string | null;
  color: string | null;
  sortOrder: number;
};

type StoredTag = { id: string; name: string; color: string | null; category_id: string };
type StoredCategory = {
  id: string;
  name: string;
  role: string | null;
  color: string | null;
  sort_order: number;
};

export class TagsRepository {
  readonly #store: DesktopDatabase;

  constructor(store: DesktopDatabase) {
    this.#store = store;
  }

  tags(): LocalTag[] {
    return (this.#store.db
      .prepare("SELECT id, name, color, category_id FROM tags_cache WHERE deleted_at IS NULL ORDER BY name ASC")
      .all() as StoredTag[]).map(tagFromStored);
  }

  categories(): LocalTagCategory[] {
    return (this.#store.db
      .prepare(
        "SELECT id, name, role, color, sort_order FROM tag_categories_cache WHERE deleted_at IS NULL ORDER BY sort_order ASC",
      )
      .all() as StoredCategory[]).map(categoryFromStored);
  }

  createTag(categoryId: string, name: string, color: string | null): LocalTag {
    const id = uuidV7();
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("INSERT INTO tags_cache (id, name, color, category_id, hlc, sync_state) VALUES (?, ?, ?, ?, ?, 'pending')")
        .run(id, name, color, categoryId, stamp);
      enqueueMutation(this.#store.db, "createTag", { id, categoryId, name, color }, stamp);
    });
    return this.#requiredTag(id);
  }

  createCategory(name: string, color: string | null): LocalTagCategory {
    const id = uuidV7();
    const stamp = this.#store.hlc.now();
    const order = Number(
      (this.#store.db.prepare("SELECT COALESCE(MAX(sort_order), -1) + 1 AS value FROM tag_categories_cache").get() as { value: number }).value,
    );
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(
          "INSERT INTO tag_categories_cache (id, name, role, color, sort_order, hlc, sync_state) VALUES (?, ?, NULL, ?, ?, ?, 'pending')",
        )
        .run(id, name, color, order, stamp);
      enqueueMutation(this.#store.db, "createTagCategory", { id, name, color, sortOrder: order }, stamp);
    });
    return this.#requiredCategory(id);
  }

  renameCategory(id: string, name: string): LocalTagCategory {
    this.#mutate("tag_categories_cache", id, "name", name, "renameTagCategory", { id, name });
    return this.#requiredCategory(id);
  }

  setCategoryColor(id: string, color: string | null): LocalTagCategory {
    this.#mutate("tag_categories_cache", id, "color", color, "setTagCategoryColor", { id, color });
    return this.#requiredCategory(id);
  }

  reorderCategories(order: Array<{ id: string; sortOrder: number }>): boolean {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      for (const row of order) {
        if (!row.id || !Number.isInteger(row.sortOrder)) throw new Error("invalid category order entry");
        this.#store.db
          .prepare("UPDATE tag_categories_cache SET sort_order = ?, hlc = ?, sync_state = 'pending' WHERE id = ?")
          .run(row.sortOrder, stamp, row.id);
      }
      enqueueMutation(this.#store.db, "reorderTagCategories", { order }, stamp);
    });
    return true;
  }

  deleteCategory(id: string): boolean {
    const row = this.#store.db.prepare("SELECT role FROM tag_categories_cache WHERE id = ?").get(id) as
      | { role: string | null }
      | undefined;
    if (!row) throw new Error("tag category not found");
    if (row.role !== null) throw new Error("default tag categories cannot be deleted");
    this.#tombstone("tag_categories_cache", id, "deleteTagCategory", { id });
    return true;
  }

  renameTag(id: string, name: string): LocalTag {
    this.#mutate("tags_cache", id, "name", name, "renameTag", { id, name });
    return this.#requiredTag(id);
  }

  setTagColor(id: string, color: string | null): LocalTag {
    this.#mutate("tags_cache", id, "color", color, "setTagColor", { id, color });
    return this.#requiredTag(id);
  }

  deleteTag(id: string): boolean {
    this.#tombstone("tags_cache", id, "deleteTag", { id });
    return true;
  }

  mergeTags(fromId: string, intoId: string): boolean {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare("UPDATE tags_cache SET deleted_at = datetime('now'), hlc = ?, sync_state = 'pending' WHERE id = ?")
        .run(stamp, fromId);
      enqueueMutation(this.#store.db, "mergeTags", { fromId, intoId }, stamp);
      const rows = this.#store.db.prepare("SELECT id, tag_ids FROM journal_entries").all() as Array<{
        id: string;
        tag_ids: string;
      }>;
      for (const row of rows) {
        const ids = stringArray(row.tag_ids);
        if (!ids.includes(fromId)) continue;
        const replaced = [...new Set(ids.map((id) => (id === fromId ? intoId : id)))];
        this.#store.db.prepare("UPDATE journal_entries SET tag_ids = ? WHERE id = ?").run(JSON.stringify(replaced), row.id);
      }
    });
    return true;
  }

  #mutate(
    table: "tag_categories_cache" | "tags_cache",
    id: string,
    column: "name" | "color",
    value: string | null,
    mutation: string,
    args: Record<string, unknown>,
  ): void {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(`UPDATE ${table} SET ${column} = ?, hlc = ?, sync_state = 'pending' WHERE id = ?`)
        .run(value, stamp, id);
      enqueueMutation(this.#store.db, mutation, args, stamp);
    });
  }

  #tombstone(
    table: "tag_categories_cache" | "tags_cache",
    id: string,
    mutation: string,
    args: Record<string, unknown>,
  ): void {
    const stamp = this.#store.hlc.now();
    transaction(this.#store.db, () => {
      this.#store.db
        .prepare(`UPDATE ${table} SET deleted_at = datetime('now'), hlc = ?, sync_state = 'pending' WHERE id = ?`)
        .run(stamp, id);
      enqueueMutation(this.#store.db, mutation, args, stamp);
    });
  }

  #requiredTag(id: string): LocalTag {
    const row = this.#store.db.prepare("SELECT id, name, color, category_id FROM tags_cache WHERE id = ?").get(id) as
      | StoredTag
      | undefined;
    if (!row) throw new Error("tag not found");
    return tagFromStored(row);
  }

  #requiredCategory(id: string): LocalTagCategory {
    const row = this.#store.db
      .prepare("SELECT id, name, role, color, sort_order FROM tag_categories_cache WHERE id = ?")
      .get(id) as StoredCategory | undefined;
    if (!row) throw new Error("tag category not found");
    return categoryFromStored(row);
  }
}

function tagFromStored(row: StoredTag): LocalTag {
  return { id: row.id, name: row.name, color: row.color, categoryId: row.category_id };
}

function categoryFromStored(row: StoredCategory): LocalTagCategory {
  return { id: row.id, name: row.name, role: row.role, color: row.color, sortOrder: row.sort_order };
}

function stringArray(value: string): string[] {
  try {
    const parsed: unknown = JSON.parse(value);
    return Array.isArray(parsed) ? parsed.filter((item): item is string => typeof item === "string") : [];
  } catch {
    return [];
  }
}
