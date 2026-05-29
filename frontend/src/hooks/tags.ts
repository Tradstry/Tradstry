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

// ---------------------------------------------------------------------------
// Query keys
// ---------------------------------------------------------------------------

const TAGS_KEY = ["tags"] as const;

const categoriesKey = () => [...TAGS_KEY, "categories"] as const;

const tagsKey = (categoryId?: string) =>
  [...TAGS_KEY, "list", categoryId ?? null] as const;

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
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: categoriesKey() });
    },
  });
}

export function useRenameTagCategory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      tagsService.renameTagCategory(fetcher, id, name),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: categoriesKey() });
    },
  });
}

export function useSetTagCategoryColor() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, color }: { id: string; color: string | null }) =>
      tagsService.setTagCategoryColor(fetcher, id, color),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: categoriesKey() });
    },
  });
}

export function useReorderTagCategories() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (order: ReorderTagCategoryItem[]) =>
      tagsService.reorderTagCategories(fetcher, order),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: categoriesKey() });
    },
  });
}

export function useDeleteTagCategory() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => tagsService.deleteTagCategory(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: TAGS_KEY });
    },
  });
}

// ---------------------------------------------------------------------------
// Tag mutation hooks
// ---------------------------------------------------------------------------

export function useCreateTag() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({
      categoryId,
      name,
      color,
    }: {
      categoryId: string;
      name: string;
      color?: string | null;
    }) => tagsService.createTag(fetcher, categoryId, name, color),
    onSuccess: (created) => {
      // Invalidate the specific category list and the all-tags list.
      queryClient.invalidateQueries({ queryKey: tagsKey(created.categoryId) });
      queryClient.invalidateQueries({ queryKey: tagsKey() });
    },
  });
}

export function useRenameTag() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, name }: { id: string; name: string }) =>
      tagsService.renameTag(fetcher, id, name),
    onSuccess: (updated) => {
      queryClient.invalidateQueries({ queryKey: tagsKey(updated.categoryId) });
      queryClient.invalidateQueries({ queryKey: tagsKey() });
    },
  });
}

export function useSetTagColor() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ id, color }: { id: string; color: string | null }) =>
      tagsService.setTagColor(fetcher, id, color),
    onSuccess: (updated) => {
      queryClient.invalidateQueries({ queryKey: tagsKey(updated.categoryId) });
      queryClient.invalidateQueries({ queryKey: tagsKey() });
    },
  });
}

export function useDeleteTag() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => tagsService.deleteTag(fetcher, id),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: TAGS_KEY });
    },
  });
}

export function useMergeTags() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: ({ fromId, intoId }: { fromId: string; intoId: string }) =>
      tagsService.mergeTags(fetcher, fromId, intoId),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: TAGS_KEY });
    },
  });
}
