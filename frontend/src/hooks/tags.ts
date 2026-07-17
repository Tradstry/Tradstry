"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as tagsService from "@/lib/service/tags";
import type {
  ReorderTagCategoryItem,
  Tag,
  TagCategory,
} from "@/lib/types/tags";
import {
  optimisticCreate,
  optimisticList,
  optimisticRemove,
  optimisticUpdate,
  tempId,
} from "./optimistic";

// ---------------------------------------------------------------------------
// Query keys
// ---------------------------------------------------------------------------

const TAGS_KEY = ["tags"] as const;

const categoriesKey = () => [...TAGS_KEY, "categories"] as const;

/** Narrow prefix covering every per-category tag list; deliberately excludes categories. */
const tagsListKey = [...TAGS_KEY, "list"] as const;

const tagsKey = (categoryId?: string) =>
  [...TAGS_KEY, "list", categoryId ?? null] as const;

const now = () => new Date().toISOString();

// ---------------------------------------------------------------------------
// Query hooks
// ---------------------------------------------------------------------------

export function useTagCategories() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<TagCategory[]>({
    queryKey: categoriesKey(),
    queryFn: () => tagsService.fetchTagCategories(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useTags(categoryId?: string) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<Tag[]>({
    queryKey: tagsKey(categoryId),
    queryFn: () => tagsService.fetchTags(fetcher, categoryId),
    enabled: isLoaded && isSignedIn,
  });
}

/** Fetch all tags (no category filter). */
export function useAllTags() {
  return useTags();
}

// ---------------------------------------------------------------------------
// Tag category mutation hooks
// ---------------------------------------------------------------------------

export function useCreateTagCategory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ name, color }: { name: string; color?: string | null }) =>
      tagsService.createTagCategory(fetcher, name, color),
    ...optimisticCreate<{ name: string; color?: string | null }, TagCategory>(
      queryClient,
      categoriesKey(),
      ({ name, color }) => ({
        id: tempId(),
        userId: "",
        name,
        role: null,
        color: color ?? null,
        sortOrder: 0,
        createdAt: now(),
        updatedAt: now(),
      }),
    ),
  });
}

export function useRenameTagCategory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      tagsService.renameTagCategory(fetcher, id, name),
    ...optimisticUpdate<{ id: string; name: string }, TagCategory>(
      queryClient,
      categoriesKey(),
      (vars) => vars.id,
      (entity, { name }) => ({ ...entity, name }),
    ),
  });
}

export function useSetTagCategoryColor() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, color }: { id: string; color: string | null }) =>
      tagsService.setTagCategoryColor(fetcher, id, color),
    ...optimisticUpdate<{ id: string; color: string | null }, TagCategory>(
      queryClient,
      categoriesKey(),
      (vars) => vars.id,
      (entity, { color }) => ({ ...entity, color }),
    ),
  });
}

export function useReorderTagCategories() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (order: ReorderTagCategoryItem[]) =>
      tagsService.reorderTagCategories(fetcher, order),
    ...optimisticList<ReorderTagCategoryItem[], TagCategory>(
      queryClient,
      categoriesKey(),
      (list, order) => {
        const rank = new Map(order.map((o, i) => [o.id, i]));
        return [...list].sort(
          (a, b) =>
            (rank.get(a.id) ?? a.sortOrder) - (rank.get(b.id) ?? b.sortOrder),
        );
      },
    ),
  });
}

export function useDeleteTagCategory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => tagsService.deleteTagCategory(fetcher, id),
    // Deleting a category cascades to its tags, so reconcile the whole tags tree.
    ...optimisticRemove<string>(
      queryClient,
      categoriesKey(),
      (id) => id,
      TAGS_KEY,
    ),
  });
}

// ---------------------------------------------------------------------------
// Tag mutation hooks
// ---------------------------------------------------------------------------

export function useCreateTag() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type CreateTagVars = {
    categoryId: string;
    name: string;
    color?: string | null;
  };
  return useMutation({
    mutationFn: ({ categoryId, name, color }: CreateTagVars) =>
      tagsService.createTag(fetcher, categoryId, name, color),
    ...optimisticCreate<CreateTagVars, Tag>(
      queryClient,
      tagsListKey,
      ({ categoryId, name, color }) => ({
        id: tempId(),
        userId: "",
        categoryId,
        name,
        color: color ?? null,
        createdAt: now(),
        updatedAt: now(),
      }),
      TAGS_KEY,
    ),
  });
}

export function useRenameTag() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      tagsService.renameTag(fetcher, id, name),
    ...optimisticUpdate<{ id: string; name: string }, Tag>(
      queryClient,
      tagsListKey,
      (vars) => vars.id,
      (entity, { name }) => ({ ...entity, name }),
      TAGS_KEY,
    ),
  });
}

export function useSetTagColor() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, color }: { id: string; color: string | null }) =>
      tagsService.setTagColor(fetcher, id, color),
    ...optimisticUpdate<{ id: string; color: string | null }, Tag>(
      queryClient,
      tagsListKey,
      (vars) => vars.id,
      (entity, { color }) => ({ ...entity, color }),
      TAGS_KEY,
    ),
  });
}

export function useDeleteTag() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => tagsService.deleteTag(fetcher, id),
    ...optimisticRemove<string>(queryClient, tagsListKey, (id) => id, TAGS_KEY),
  });
}

export function useMergeTags() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ fromId, intoId }: { fromId: string; intoId: string }) =>
      tagsService.mergeTags(fetcher, fromId, intoId),
    // Merge folds one tag into another and re-points trade links; drop the source now,
    // let the settle refetch bring the reconciled trade-tag state.
    ...optimisticRemove<{ fromId: string; intoId: string }>(
      queryClient,
      tagsListKey,
      (vars) => vars.fromId,
      TAGS_KEY,
    ),
  });
}
