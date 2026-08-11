import { createStore } from 'zustand/vanilla';
import type { DecryptedOverview, Draft, TaskReceipt, VaultStateUpdate } from './types';

/** Sync indicator state (ui-state-charts §6). */
export interface SyncState {
  isSyncing: boolean;
  /** Integer 0-100 percentage (data.md §1 rule 5). */
  progress: number;
}

/** The normalized overview map + ephemeral draft slice. data.md §5. */
export interface VaultStoreState {
  /** Normalized `Uuid -> DecryptedOverview` map. O(1) renders. */
  items: Readonly<Record<string, DecryptedOverview>>;
  /** Ephemeral plaintext drafts keyed by `TaskReceipt`. In-memory ONLY. */
  drafts: Readonly<Record<string, Draft>>;
  isLocked: boolean;
  /** Read-only gate driven by `KeyUpdateRequired`. */
  isReadOnly: boolean;
  sync: SyncState;
  /** Most recent conflict event (for resolution UI). */
  lastConflict: { uuid: string } | null;

  // --- Overview actions ---
  upsertOverview(overview: DecryptedOverview): void;
  deleteOverview(uuid: string): void;

  // --- Draft slice (TaskReceipt-keyed) ---
  setDraft(receipt: TaskReceipt, draft: Draft): void;
  getDraft(receipt: TaskReceipt): Draft | undefined;
  clearDraft(receipt: TaskReceipt): void;
  wipeDrafts(): void;

  // --- Lifecycle / sync ---
  lock(): void;
  unlock(): void;
  setSync(patch: Partial<SyncState>): void;

  /** Reduce a `VaultStateUpdate` event into the store (ui-state-charts §6). */
  applyUpdate(update: VaultStateUpdate): void;
}

export const vaultStore = createStore<VaultStoreState>((set, get) => ({
  items: {},
  drafts: {},
  isLocked: true,
  isReadOnly: false,
  sync: { isSyncing: false, progress: 0 },
  lastConflict: null,

  upsertOverview(overview) {
    set((state) => ({ items: { ...state.items, [overview.uuid]: overview } }));
  },

  deleteOverview(uuid) {
    set((state) => {
      const { [uuid]: _removed, ...rest } = state.items;
      return { items: rest };
    });
  },

  setDraft(receipt, draft) {
    set((state) => ({ drafts: { ...state.drafts, [receipt]: draft } }));
  },

  getDraft(receipt) {
    return get().drafts[receipt];
  },

  clearDraft(receipt) {
    set((state) => {
      const { [receipt]: _removed, ...rest } = state.drafts;
      return { drafts: rest };
    });
  },

  wipeDrafts() {
    set({ drafts: {} });
  },

  lock() {
    set({
      items: {},
      drafts: {},
      isLocked: true,
      isReadOnly: false,
      sync: { isSyncing: false, progress: 0 },
      lastConflict: null,
    });
  },

  unlock() {
    set({ isLocked: false });
  },

  setSync(patch) {
    set((state) => ({ sync: { ...state.sync, ...patch } }));
  },

  applyUpdate(update) {
    const { upsertOverview, deleteOverview, clearDraft, setSync, wipeDrafts } = get();
    switch (update.type) {
      case 'SyncStarted':
        setSync({ isSyncing: true });
        break;
      case 'SyncProgress':
        setSync({ isSyncing: true, progress: update.progressPercentage });
        break;
      case 'SyncCompleted':
        setSync({ isSyncing: false, progress: 0 });
        break;
      case 'SyncFailed':
        setSync({ isSyncing: false });
        break;
      case 'OverviewUpserted':
        upsertOverview(update.overview);
        break;
      case 'OverviewDeleted':
        deleteOverview(update.uuid);
        break;
      case 'ConflictDetected':
        set({ lastConflict: { uuid: update.event.uuid } });
        break;
      case 'KeyUpdateRequired':
        set({ isReadOnly: true });
        break;
      case 'VaultLocked':
        wipeDrafts();
        set({ items: {}, isLocked: true, isReadOnly: false });
        break;
      case 'NewerVersionAvailable':
        // List-level "has update" pill is driven by the component; no state change.
        break;
      case 'MutationSucceeded':
        // Discard the draft for this receipt.
        clearDraft(update.receipt);
        break;
      case 'MutationFailed':
        // Surgically revert the normalized map using originalState. The draft is
        // intentionally retained so the recovery flow can re-inject it.
        if (update.originalState.type === 'Saved') {
          upsertOverview(update.originalState.overview);
        } else {
          upsertOverview(update.originalState.overview);
        }
        break;
    }
  },
}));

/** Convenience subscription helper mirroring `useStore` selector semantics. */
export function getOverview(uuid: string): DecryptedOverview | undefined {
  return vaultStore.getState().items[uuid];
}

export function allOverviews(): readonly DecryptedOverview[] {
  return Object.values(vaultStore.getState().items);
}

export type VaultStore = typeof vaultStore;
