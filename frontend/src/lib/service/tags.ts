import type { GraphQLFetcher } from "@/lib/client";
import type {
  ReorderTagCategoryItem,
  Tag,
  TagCategory,
} from "@/lib/types/tags";

// ---------------------------------------------------------------------------
// Fragments
// ---------------------------------------------------------------------------

const TAG_CATEGORY_FIELDS = `
  id
  userId
  name
  role
  color
  sortOrder
  createdAt
  updatedAt
`;

const TAG_FIELDS = `
  id
  userId
  categoryId
  name
  color
  createdAt
  updatedAt
`;

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

const TAG_CATEGORIES_QUERY = `
  query TagCategories {
    tagCategories {
      ${TAG_CATEGORY_FIELDS}
    }
  }
`;

const TAGS_QUERY = `
  query Tags($categoryId: String) {
    tags(categoryId: $categoryId) {
      ${TAG_FIELDS}
    }
  }
`;

// ---------------------------------------------------------------------------
// Category mutations
// ---------------------------------------------------------------------------

const CREATE_TAG_CATEGORY_MUTATION = `
  mutation CreateTagCategory($name: String!, $color: String) {
    createTagCategory(name: $name, color: $color) {
      ${TAG_CATEGORY_FIELDS}
    }
  }
`;

const RENAME_TAG_CATEGORY_MUTATION = `
  mutation RenameTagCategory($id: String!, $name: String!) {
    renameTagCategory(id: $id, name: $name) {
      ${TAG_CATEGORY_FIELDS}
    }
  }
`;

const SET_TAG_CATEGORY_COLOR_MUTATION = `
  mutation SetTagCategoryColor($id: String!, $color: String) {
    setTagCategoryColor(id: $id, color: $color) {
      ${TAG_CATEGORY_FIELDS}
    }
  }
`;

const REORDER_TAG_CATEGORIES_MUTATION = `
  mutation ReorderTagCategories($order: [ReorderTagCategoryInput!]!) {
    reorderTagCategories(order: $order)
  }
`;

const DELETE_TAG_CATEGORY_MUTATION = `
  mutation DeleteTagCategory($id: String!) {
    deleteTagCategory(id: $id)
  }
`;

// ---------------------------------------------------------------------------
// Tag mutations
// ---------------------------------------------------------------------------

const CREATE_TAG_MUTATION = `
  mutation CreateTag($categoryId: String!, $name: String!, $color: String) {
    createTag(categoryId: $categoryId, name: $name, color: $color) {
      ${TAG_FIELDS}
    }
  }
`;

const RENAME_TAG_MUTATION = `
  mutation RenameTag($id: String!, $name: String!) {
    renameTag(id: $id, name: $name) {
      ${TAG_FIELDS}
    }
  }
`;

const SET_TAG_COLOR_MUTATION = `
  mutation SetTagColor($id: String!, $color: String) {
    setTagColor(id: $id, color: $color) {
      ${TAG_FIELDS}
    }
  }
`;

const DELETE_TAG_MUTATION = `
  mutation DeleteTag($id: String!) {
    deleteTag(id: $id)
  }
`;

const MERGE_TAGS_MUTATION = `
  mutation MergeTags($fromId: String!, $intoId: String!) {
    mergeTags(fromId: $fromId, intoId: $intoId)
  }
`;

// ---------------------------------------------------------------------------
// Fetchers — queries
// ---------------------------------------------------------------------------

export async function fetchTagCategories(
  fetcher: GraphQLFetcher,
): Promise<TagCategory[]> {
  const data = await fetcher<{ tagCategories: TagCategory[] }>(
    TAG_CATEGORIES_QUERY,
  );
  return data.tagCategories;
}

export async function fetchTags(
  fetcher: GraphQLFetcher,
  categoryId?: string,
): Promise<Tag[]> {
  const data = await fetcher<{ tags: Tag[] }>(TAGS_QUERY, {
    categoryId: categoryId ?? null,
  });
  return data.tags;
}

// ---------------------------------------------------------------------------
// Fetchers — category mutations
// ---------------------------------------------------------------------------

export async function createTagCategory(
  fetcher: GraphQLFetcher,
  name: string,
  color?: string | null,
): Promise<TagCategory> {
  const data = await fetcher<{ createTagCategory: TagCategory }>(
    CREATE_TAG_CATEGORY_MUTATION,
    { name, color: color ?? null },
  );
  return data.createTagCategory;
}

export async function renameTagCategory(
  fetcher: GraphQLFetcher,
  id: string,
  name: string,
): Promise<TagCategory> {
  const data = await fetcher<{ renameTagCategory: TagCategory }>(
    RENAME_TAG_CATEGORY_MUTATION,
    { id, name },
  );
  return data.renameTagCategory;
}

export async function setTagCategoryColor(
  fetcher: GraphQLFetcher,
  id: string,
  color: string | null,
): Promise<TagCategory> {
  const data = await fetcher<{ setTagCategoryColor: TagCategory }>(
    SET_TAG_CATEGORY_COLOR_MUTATION,
    { id, color },
  );
  return data.setTagCategoryColor;
}

export async function reorderTagCategories(
  fetcher: GraphQLFetcher,
  order: ReorderTagCategoryItem[],
): Promise<boolean> {
  const data = await fetcher<{ reorderTagCategories: boolean }>(
    REORDER_TAG_CATEGORIES_MUTATION,
    { order },
  );
  return data.reorderTagCategories;
}

export async function deleteTagCategory(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteTagCategory: boolean }>(
    DELETE_TAG_CATEGORY_MUTATION,
    { id },
  );
  return data.deleteTagCategory;
}

// ---------------------------------------------------------------------------
// Fetchers — tag mutations
// ---------------------------------------------------------------------------

export async function createTag(
  fetcher: GraphQLFetcher,
  categoryId: string,
  name: string,
  color?: string | null,
): Promise<Tag> {
  const data = await fetcher<{ createTag: Tag }>(CREATE_TAG_MUTATION, {
    categoryId,
    name,
    color: color ?? null,
  });
  return data.createTag;
}

export async function renameTag(
  fetcher: GraphQLFetcher,
  id: string,
  name: string,
): Promise<Tag> {
  const data = await fetcher<{ renameTag: Tag }>(RENAME_TAG_MUTATION, {
    id,
    name,
  });
  return data.renameTag;
}

export async function setTagColor(
  fetcher: GraphQLFetcher,
  id: string,
  color: string | null,
): Promise<Tag> {
  const data = await fetcher<{ setTagColor: Tag }>(SET_TAG_COLOR_MUTATION, {
    id,
    color,
  });
  return data.setTagColor;
}

export async function deleteTag(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteTag: boolean }>(DELETE_TAG_MUTATION, {
    id,
  });
  return data.deleteTag;
}

export async function mergeTags(
  fetcher: GraphQLFetcher,
  fromId: string,
  intoId: string,
): Promise<boolean> {
  const data = await fetcher<{ mergeTags: boolean }>(MERGE_TAGS_MUTATION, {
    fromId,
    intoId,
  });
  return data.mergeTags;
}
