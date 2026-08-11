import { vaultStore } from './store';
import type { VaultStateUpdate } from './types';

/** A subscription that receives every `VaultStateUpdate` event. */
export type UpdateListener = (update: VaultStateUpdate) => void;

export type Unsubscribe = () => void;

/**
 * Minimal, typed, synchronous event bus for `VaultStateUpdate` events
 * (data.md §6.3). The Core pushes reactive diffs here; the UI never polls for
 * lists (client.md §5).
 */
export class VaultEventBus {
  private listeners = new Set<UpdateListener>();

  /** Register a listener. Returns an unsubscribe function. */
  subscribe(listener: UpdateListener): Unsubscribe {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  /** Forward an event to every subscriber. */
  emit(update: VaultStateUpdate): void {
    for (const listener of this.listeners) {
      listener(update);
    }
  }

  /** Remove all listeners (e.g. on vault lock / worker teardown). */
  clear(): void {
    this.listeners.clear();
  }
}

/**
 * Default singleton event bus used across the web client.
 */
export const vaultEventBus = new VaultEventBus();

/** Subscribe the shared store to the shared event bus. Returns unsubscribe. */
export function attachStoreToEventBus(bus: VaultEventBus = vaultEventBus): Unsubscribe {
  return bus.subscribe((update) => vaultStore.getState().applyUpdate(update));
}

/** Subscribe the shared event bus to a client's event stream. */
export function attachClientToEventBus(
  onEvent: (listener: UpdateListener) => Unsubscribe,
  bus: VaultEventBus = vaultEventBus,
): Unsubscribe {
  return onEvent((update) => bus.emit(update));
}
