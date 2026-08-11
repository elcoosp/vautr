import { useStore } from 'zustand/react';
import type { SyncState } from './store';
import { vaultStore } from './store';
import type { DecryptedOverview, Draft, TaskReceipt } from './types';

/**
 * React bindings over the vanilla `vaultStore`. These are thin, reactive
 * projectors; the store itself stays framework-agnostic (ui-state-charts §6).
 */

/** All overviews as an array, reactive. */
export function useOverviews(): readonly DecryptedOverview[] {
  return useStore(vaultStore, (s) => Object.values(s.items));
}

/** The overview for `uuid`, reactive. */
export function useOverview(uuid: string): DecryptedOverview | undefined {
  return useStore(vaultStore, (s) => s.items[uuid]);
}

export function useIsLocked(): boolean {
  return useStore(vaultStore, (s) => s.isLocked);
}

export function useIsReadOnly(): boolean {
  return useStore(vaultStore, (s) => s.isReadOnly);
}

export function useSyncState(): SyncState {
  return useStore(vaultStore, (s) => s.sync);
}

/** The ephemeral draft for `receipt`, reactive. */
export function useDraft(receipt: TaskReceipt): Draft | undefined {
  return useStore(vaultStore, (s) => s.drafts[receipt]);
}

/** Actions are stable references; expose them for convenience. */
export function useVaultActions() {
  return useStore(vaultStore, (s) => ({
    upsertOverview: s.upsertOverview,
    deleteOverview: s.deleteOverview,
    setDraft: s.setDraft,
    clearDraft: s.clearDraft,
    applyUpdate: s.applyUpdate,
    lock: s.lock,
    unlock: s.unlock,
  }));
}
