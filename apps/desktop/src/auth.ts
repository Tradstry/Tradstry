const invoke = window.tradstry.invoke;

export type AuthStatus = {
  signedIn: boolean;
  email?: string | null;
  name?: string | null;
};

/** Read current auth state from the keychain (refreshes if the token expired). */
export const getAuthStatus = () => invoke<AuthStatus>("auth_status");

/** Launch the browser OAuth flow; resolves when sign-in completes. */
export const signIn = () => invoke<AuthStatus>("sign_in");

/** Clear the stored session from the keychain. */
export const signOut = () => invoke<void>("sign_out");
