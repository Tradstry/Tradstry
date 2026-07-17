"use client";

import type { QueryClient, QueryKey } from "@tanstack/react-query";

/**
 * Optimistic-update helpers for list caches.
 *
 * The app's mutations used to write to the server and then refetch the whole list before the
 * UI moved — two round-trips per click. These factories instead patch the cache in `onMutate`
 * so the change shows in the same frame, roll back in `onError`, and reconcile with the server
 * in the background in `onSettled`. The user never waits on the network to see their own action.
 *
 * They operate on every ARRAY cache under a prefix key via `setQueriesData`, so a single call
 * covers `["journal"]`, `["journal","account",id]`, etc. Scalar caches (a single-entity query)
 * are left untouched by the patch and picked up by the settle-time invalidation.
 *
 * The handlers deliberately carry no `TData` generic, so spreading them into `useMutation`
 * leaves the mutation's result type to be inferred from `mutationFn` — deletes can return a
 * boolean while creates return the entity, and both work.
 */

type Identified = { id: string };

export type OptimisticContext = {
  snapshots: [QueryKey, unknown][];
};

type Handlers<TVars> = {
  onMutate: (vars: TVars) => Promise<OptimisticContext>;
  onError: (
    error: Error,
    vars: TVars,
    ctx: OptimisticContext | undefined,
  ) => void;
  onSettled: () => void;
};

function patchLists<T>(
  qc: QueryClient,
  prefix: QueryKey,
  fn: (list: T[]) => T[],
): void {
  qc.setQueriesData<unknown>({ queryKey: prefix }, (old: unknown) =>
    Array.isArray(old) ? fn(old as T[]) : old,
  );
}

async function snapshot(
  qc: QueryClient,
  prefix: QueryKey,
): Promise<OptimisticContext> {
  // Stop in-flight refetches so they can't clobber the optimistic write mid-flight.
  await qc.cancelQueries({ queryKey: prefix });
  return {
    snapshots: qc
      .getQueriesData({ queryKey: prefix })
      .map(([key, data]) => [key, data] as [QueryKey, unknown]),
  };
}

function rollback(qc: QueryClient, ctx?: OptimisticContext): void {
  for (const [key, data] of ctx?.snapshots ?? []) {
    qc.setQueryData(key, data);
  }
}

function reconcile(qc: QueryClient, prefix: QueryKey): void {
  qc.invalidateQueries({ queryKey: prefix });
}

/** Insert a temporary entity at the top of every matching list; the settle refetch swaps in the real one. */
export function optimisticCreate<TVars, T extends Identified>(
  qc: QueryClient,
  prefix: QueryKey,
  makeTemp: (vars: TVars) => T,
  reconcileKey: QueryKey = prefix,
): Handlers<TVars> {
  return {
    onMutate: async (vars) => {
      const ctx = await snapshot(qc, prefix);
      patchLists<T>(qc, prefix, (list) => [makeTemp(vars), ...list]);
      return ctx;
    },
    onError: (_error, _vars, ctx) => rollback(qc, ctx),
    onSettled: () => reconcile(qc, reconcileKey),
  };
}

/** Patch a matching entity by id across every list. */
export function optimisticUpdate<TVars, T extends Identified>(
  qc: QueryClient,
  prefix: QueryKey,
  idOf: (vars: TVars) => string,
  patch: (entity: T, vars: TVars) => T,
  reconcileKey: QueryKey = prefix,
): Handlers<TVars> {
  return {
    onMutate: async (vars) => {
      const ctx = await snapshot(qc, prefix);
      const id = idOf(vars);
      patchLists<T>(qc, prefix, (list) =>
        list.map((entity) => (entity.id === id ? patch(entity, vars) : entity)),
      );
      return ctx;
    },
    onError: (_error, _vars, ctx) => rollback(qc, ctx),
    onSettled: () => reconcile(qc, reconcileKey),
  };
}

/** Apply a whole-list transform (reordering, bulk edits) across every matching list. */
export function optimisticList<TVars, T extends Identified>(
  qc: QueryClient,
  prefix: QueryKey,
  transform: (list: T[], vars: TVars) => T[],
  reconcileKey: QueryKey = prefix,
): Handlers<TVars> {
  return {
    onMutate: async (vars) => {
      const ctx = await snapshot(qc, prefix);
      patchLists<T>(qc, prefix, (list) => transform(list, vars));
      return ctx;
    },
    onError: (_error, _vars, ctx) => rollback(qc, ctx),
    onSettled: () => reconcile(qc, reconcileKey),
  };
}

/** Drop a matching entity by id from every list. */
export function optimisticRemove<TVars>(
  qc: QueryClient,
  prefix: QueryKey,
  idOf: (vars: TVars) => string,
  reconcileKey: QueryKey = prefix,
): Handlers<TVars> {
  return {
    onMutate: async (vars) => {
      const ctx = await snapshot(qc, prefix);
      const id = idOf(vars);
      patchLists<Identified>(qc, prefix, (list) =>
        list.filter((entity) => entity.id !== id),
      );
      return ctx;
    },
    onError: (_error, _vars, ctx) => rollback(qc, ctx),
    onSettled: () => reconcile(qc, reconcileKey),
  };
}

/** A client-side id for a temp entity, replaced when the server's real row arrives. */
export function tempId(): string {
  return `temp-${crypto.randomUUID()}`;
}
