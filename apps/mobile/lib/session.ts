import { create } from 'zustand';

/** App-wide auth/session state. */
export interface SessionState {
  /** Logged-in username (null when signed out). */
  username: string | null;
  /** True once a valid session token is loaded onto the API client. */
  authenticated: boolean;
  /** True while a restore/login is in flight. */
  booting: boolean;
  setUsername: (username: string | null) => void;
  setAuthenticated: (authenticated: boolean) => void;
  setBooting: (booting: boolean) => void;
  signOut: () => void;
}

export const useSession = create<SessionState>((set) => ({
  username: null,
  authenticated: false,
  booting: true,
  setUsername: (username) => set({ username }),
  setAuthenticated: (authenticated) => set({ authenticated }),
  setBooting: (booting) => set({ booting }),
  signOut: () => set({ username: null, authenticated: false }),
}));
