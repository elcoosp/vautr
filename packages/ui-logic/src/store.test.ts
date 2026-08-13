import { describe, expect, it } from 'vitest';
import { attachStoreToEventBus, VaultEventBus } from '../src/eventBus';
import { type VaultStoreState, vaultStore } from '../src/store';
import type { ConflictEvent, DecryptedOverview, Draft, VaultStateUpdate } from '../src/types';

function resetStore(): void {
  vaultStore.setState({ items: {}, drafts: {}, isLocked: true, isReadOnly: false });
}

function makeOverview(partial: Partial<DecryptedOverview> = {}): DecryptedOverview {
  return {
    uuid: '11111111-1111-4111-8111-111111111111',
    title: 'Acme Bank',
    subtitle: 'login@acme.example',
    iconKey: 'bank',
    urls: ['https://acme.example'],
    updatedAt: 1720000000000,
    ...partial,
  };
}

function makeDraft(): Draft {
  return {
    uuid: '11111111-1111-4111-8111-111111111111',
    syncEpoch: '1',
    title: 'Acme Bank',
    subtitle: 'login@acme.example',
    iconKey: 'bank',
    urls: [],
    password: 'hunter2-example',
    notes: 'local edit',
  };
}

function state(): VaultStoreState {
  return vaultStore.getState();
}

describe('overview map', () => {
  it('upserts an overview then retrieves it', () => {
    resetStore();
    const overview = makeOverview();
    vaultStore.getState().upsertOverview(overview);
    expect(state().items[overview.uuid]).toEqual(overview);
  });

  it('upserting the same uuid replaces the existing entry', () => {
    resetStore();
    vaultStore.getState().upsertOverview(makeOverview());
    vaultStore
      .getState()
      .upsertOverview(makeOverview({ title: 'Acme Bank v2', updatedAt: 1720000000001 }));
    const item = state().items['11111111-1111-4111-8111-111111111111'];
    expect(item?.title).toBe('Acme Bank v2');
    expect(state().items).toHaveProperty('11111111-1111-4111-8111-111111111111');
  });

  it('deletes an overview', () => {
    resetStore();
    vaultStore.getState().upsertOverview(makeOverview());
    vaultStore.getState().deleteOverview('11111111-1111-4111-8111-111111111111');
    expect(state().items['11111111-1111-4111-8111-111111111111']).toBeUndefined();
  });

  it('deleting an unknown uuid is a no-op', () => {
    resetStore();
    vaultStore.getState().upsertOverview(makeOverview());
    vaultStore.getState().deleteOverview('22222222-2222-4222-8222-222222222222');
    expect(Object.keys(state().items)).toHaveLength(1);
  });
});

describe('draft slice', () => {
  it('sets a draft by receipt and gets it back', () => {
    resetStore();
    const receipt = '42';
    vaultStore.getState().setDraft(receipt, makeDraft());
    expect(state().getDraft(receipt)?.password).toBe('hunter2-example');
  });

  it('returns undefined for an unknown receipt', () => {
    resetStore();
    expect(state().getDraft('nope')).toBeUndefined();
  });

  it('clears a draft', () => {
    resetStore();
    const receipt = '7';
    vaultStore.getState().setDraft(receipt, makeDraft());
    vaultStore.getState().clearDraft(receipt);
    expect(state().getDraft(receipt)).toBeUndefined();
  });

  it('wipes all drafts on lock', () => {
    resetStore();
    vaultStore.getState().setDraft('1', makeDraft());
    vaultStore.getState().setDraft('2', makeDraft());
    vaultStore.getState().lock();
    expect(state().drafts).toEqual({});
  });
});

describe('event bus -> store reduction (MutationFailed draft recovery)', () => {
  it('reduces OverviewUpserted and OverviewDeleted through the bus', () => {
    resetStore();
    const bus = new VaultEventBus();
    const unsub = attachStoreToEventBus(bus);
    const overview = makeOverview();
    bus.emit({ type: 'OverviewUpserted', overview });
    expect(state().items[overview.uuid]).toEqual(overview);
    bus.emit({ type: 'OverviewDeleted', uuid: overview.uuid });
    expect(state().items[overview.uuid]).toBeUndefined();
    unsub();
  });

  it('keeps the draft and reverts the list on MutationFailed', () => {
    resetStore();
    const bus = new VaultEventBus();
    const unsub = attachStoreToEventBus(bus);

    const overview = makeOverview();
    // Start from a present list entry.
    vaultStore.getState().upsertOverview(overview);
    // Persist the user's draft pending async validation.
    const receipt = '99';
    vaultStore.getState().setDraft(receipt, makeDraft());

    // Delete fails (epoch mismatch): the list is surgically re-inserted and the
    // draft remains available for the recovery flow (ui-state-charts §4).
    const update: VaultStateUpdate = {
      type: 'MutationFailed',
      receipt,
      error: { type: 'EpochMismatch' },
      originalState: { type: 'Deleted', overview },
    };
    bus.emit(update);

    expect(state().items[overview.uuid]).toEqual(overview);
    expect(state().getDraft(receipt)?.password).toBe('hunter2-example');
    unsub();
  });

  it('drops the draft on MutationSucceeded', () => {
    resetStore();
    const bus = new VaultEventBus();
    const unsub = attachStoreToEventBus(bus);
    const receipt = '11';
    vaultStore.getState().setDraft(receipt, makeDraft());
    bus.emit({ type: 'MutationSucceeded', receipt });
    expect(state().getDraft(receipt)).toBeUndefined();
    unsub();
  });

  it('wipe drafts and locks on VaultLocked', () => {
    resetStore();
    const bus = new VaultEventBus();
    const unsub = attachStoreToEventBus(bus);
    vaultStore.getState().setDraft('5', makeDraft());
    bus.emit({ type: 'VaultLocked' });
    expect(state().isLocked).toBe(true);
    expect(state().drafts).toEqual({});
    unsub();
  });
});

// ---------------------------------------------------------------------------
// VTR-056: conflict resolution queue (data.md §7.2)
// ---------------------------------------------------------------------------

function makeConflict(
  partial: Partial<{
    uuid: string;
    localVersion: string;
    serverVersion: string;
    isToxic: boolean;
  }> = {},
): ConflictEvent {
  return {
    uuid: '33333333-3333-4333-8333-333333333333',
    localVersion: '2',
    serverVersion: '3',
    isToxic: false,
    ...partial,
  };
}

describe('conflict resolution queue (VTR-056)', () => {
  it('TDD1: a valid conflict enqueues and is shown at the head', () => {
    resetStore();
    vaultStore.getState().enqueueConflict(makeConflict());
    expect(state().conflictQueue).toHaveLength(1);
    expect(state().conflictQueue[0]?.isToxic).toBe(false);
    expect(state().conflictQueue[0]?.uuid).toBe('33333333-3333-4333-8333-333333333333');
  });

  it('TDD2: resolving "keep server" removes the conflict from the queue', () => {
    resetStore();
    const c = makeConflict();
    vaultStore.getState().enqueueConflict(c);
    vaultStore.getState().resolveConflict(c.uuid);
    expect(state().conflictQueue).toHaveLength(0);
    // The item is not parked as ignored; the DashMap entry is cleared.
    expect(state().ignoredConflicts[c.uuid]).toBeUndefined();
  });

  it('TDD3: a toxic conflict is flagged and shown with toxic text', () => {
    resetStore();
    vaultStore.getState().enqueueConflict(makeConflict({ isToxic: true }));
    const head = state().conflictQueue[0];
    expect(head?.isToxic).toBe(true);
    // The modal maps toxic -> "unreadable update" copy + Keep Local / Overwrite
    // Server buttons; the head carries the flag that drives that branch.
  });

  it('TDD4: dismissing without choosing keeps the conflict ignored (not resolved)', () => {
    resetStore();
    const c = makeConflict();
    vaultStore.getState().enqueueConflict(c);
    vaultStore.getState().dismissConflict(c);
    expect(state().conflictQueue).toHaveLength(0);
    // Dismissed -> stays in the DashMap as ignored, not resolved.
    expect(state().ignoredConflicts[c.uuid]).toEqual(c);
  });

  it('TDD5: multiple concurrent conflicts queue FIFO and resolve one at a time', () => {
    resetStore();
    const a = makeConflict({ uuid: 'a', localVersion: '1', serverVersion: '2' });
    const b = makeConflict({ uuid: 'b', localVersion: '4', serverVersion: '5' });
    const c = makeConflict({ uuid: 'c', localVersion: '7', serverVersion: '8' });
    vaultStore.getState().enqueueConflict(a);
    vaultStore.getState().enqueueConflict(b);
    vaultStore.getState().enqueueConflict(c);
    // FIFO order; only the head is shown (no overlap).
    expect(state().conflictQueue.map((e) => e.uuid)).toEqual(['a', 'b', 'c']);
    // Resolve the head -> next conflict surfaces, queue never overlaps.
    vaultStore.getState().resolveConflict('a');
    expect(state().conflictQueue.map((e) => e.uuid)).toEqual(['b', 'c']);
    vaultStore.getState().resolveConflict('b');
    expect(state().conflictQueue.map((e) => e.uuid)).toEqual(['c']);
    vaultStore.getState().resolveConflict('c');
    expect(state().conflictQueue).toHaveLength(0);
  });

  it('applies ConflictDetected through the event bus (enqueue path)', () => {
    resetStore();
    const bus = new VaultEventBus();
    const unsub = attachStoreToEventBus(bus);
    bus.emit({ type: 'ConflictDetected', event: makeConflict({ uuid: 'd' }) });
    expect(state().conflictQueue.map((e) => e.uuid)).toEqual(['d']);
    unsub();
  });
});
