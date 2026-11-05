import { create } from 'zustand';

/**
 * Vault idle-lock (VTRFIX-SEC-C06). The `_app` shell flips this to `locked`
 * when the app returns from the background after more than
 * `VAULT_IDLE_LOCK_MS`; protected surfaces (e.g. the Emergency Kit reveal in
 * settings) read it and refuse to operate while locked. Unlock happens via
 * `requireBiometric` in the app shell.
 */

/** Idle time in the background after which the vault re-locks. */
export const VAULT_IDLE_LOCK_MS = 2 * 60 * 1000;

interface VaultLockState {
  locked: boolean;
  setLocked: (locked: boolean) => void;
}

export const useVaultLock = create<VaultLockState>((set) => ({
  locked: false,
  setLocked: (locked) => set({ locked }),
}));
