import type { GraphQLFetcher } from "@/lib/client";

export interface UserPrompt {
  id: string;
  name: string;
  content: string;
  createdAt: string;
  updatedAt: string;
}

const PROMPT_FIELDS = `
  id
  name
  content
  createdAt
  updatedAt
`;

const USER_PROMPTS_QUERY = `
  query UserPrompts {
    userPrompts {
      ${PROMPT_FIELDS}
    }
  }
`;

const CREATE_PROMPT_MUTATION = `
  mutation CreateUserPrompt($name: String!, $content: String!) {
    createUserPrompt(name: $name, content: $content) {
      ${PROMPT_FIELDS}
    }
  }
`;

const UPDATE_PROMPT_MUTATION = `
  mutation UpdateUserPrompt($id: String!, $name: String, $content: String) {
    updateUserPrompt(id: $id, name: $name, content: $content) {
      ${PROMPT_FIELDS}
    }
  }
`;

const DELETE_PROMPT_MUTATION = `
  mutation DeleteUserPrompt($id: String!) {
    deleteUserPrompt(id: $id)
  }
`;

export async function fetchUserPrompts(
  fetcher: GraphQLFetcher,
): Promise<UserPrompt[]> {
  const data = await fetcher<{ userPrompts: UserPrompt[] }>(USER_PROMPTS_QUERY);
  return data.userPrompts;
}

export async function createUserPrompt(
  fetcher: GraphQLFetcher,
  name: string,
  content: string,
): Promise<UserPrompt> {
  const data = await fetcher<{ createUserPrompt: UserPrompt }>(CREATE_PROMPT_MUTATION, { name, content });
  return data.createUserPrompt;
}

export async function updateUserPrompt(
  fetcher: GraphQLFetcher,
  id: string,
  name?: string,
  content?: string,
): Promise<UserPrompt> {
  const data = await fetcher<{ updateUserPrompt: UserPrompt }>(UPDATE_PROMPT_MUTATION, { id, name, content });
  return data.updateUserPrompt;
}

export async function deleteUserPrompt(
  fetcher: GraphQLFetcher,
  id: string,
): Promise<boolean> {
  const data = await fetcher<{ deleteUserPrompt: boolean }>(DELETE_PROMPT_MUTATION, { id });
  return data.deleteUserPrompt;
}
