export type TagRole = "MISTAKE" | "TACTIC" | "EDGE";

export interface TagCategory {
  id: string;
  userId: string;
  workspaceId: string;
  name: string;
  role: TagRole | null;
  color: string | null;
  sortOrder: number;
  createdAt: string;
  updatedAt: string;
}

export interface Tag {
  id: string;
  userId: string;
  workspaceId: string;
  categoryId: string;
  name: string;
  color: string | null;
  createdAt: string;
  updatedAt: string;
}

export interface ReorderTagCategoryItem {
  id: string;
  sortOrder: number;
}
