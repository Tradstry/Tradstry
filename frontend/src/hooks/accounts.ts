"use client";

import { useAuth } from "@clerk/nextjs";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useGraphQL } from "@/lib/client";
import * as accountService from "@/lib/service/accounts";
import type {
  Account,
  CreateAccountInput,
  UpdateAccountInput,
} from "@/lib/types/accounts";
import { optimisticRemove, optimisticUpdate } from "./optimistic";

const ACCOUNTS_KEY = ["accounts"] as const;

export function useAccounts() {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<Account[]>({
    queryKey: ACCOUNTS_KEY,
    queryFn: () => accountService.fetchAccounts(fetcher),
    enabled: isLoaded && isSignedIn,
  });
}

export function useAccount(id: string | null) {
  const { isLoaded, isSignedIn } = useAuth();
  const fetcher = useGraphQL();

  return useQuery<Account | null>({
    queryKey: [...ACCOUNTS_KEY, id],
    queryFn: () => accountService.fetchAccount(fetcher, id!),
    enabled: isLoaded && isSignedIn && !!id,
  });
}

export function useCreateAccount() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (input: CreateAccountInput) =>
      accountService.createAccount(fetcher, input),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ACCOUNTS_KEY });
    },
  });
}

export function useUpdateAccount() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  type UpdateVars = { id: string; input: UpdateAccountInput };
  return useMutation({
    mutationFn: ({ id, input }: UpdateVars) =>
      accountService.updateAccount(fetcher, id, input),
    ...optimisticUpdate<UpdateVars, Account>(
      queryClient,
      ACCOUNTS_KEY,
      (vars) => vars.id,
      (entity, { input }) => ({ ...entity, ...input }) as Account,
    ),
  });
}

export function useDeleteAccount() {
  const fetcher = useGraphQL();
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (id: string) => accountService.deleteAccount(fetcher, id),
    ...optimisticRemove<string>(queryClient, ACCOUNTS_KEY, (id) => id),
  });
}
