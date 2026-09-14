import { useSyncState } from '@vautr/ui-logic';

/** Sync indicator: subtle pulse while syncing + a live progress bar. */
export function SyncIndicator() {
  const sync = useSyncState();

  if (!sync.isSyncing) {
    return (
      <span
        role="status"
        aria-label="Vault is up to date"
        className="inline-flex items-center gap-1.5 text-sm text-text-muted"
      >
        <span className="size-1.5 rounded-full bg-success" aria-hidden="true" />
        Synced
      </span>
    );
  }

  return (
    <span
      role="status"
      aria-live="polite"
      className="inline-flex items-center gap-1.5 text-sm text-text"
    >
      <span className="relative flex size-2">
        <span
          className="absolute inline-flex size-full animate-ping rounded-full bg-accent opacity-50"
          aria-hidden="true"
        />
        <span className="relative inline-flex size-2 rounded-full bg-accent" aria-hidden="true" />
      </span>
      Syncing {sync.progress}%
    </span>
  );
}
