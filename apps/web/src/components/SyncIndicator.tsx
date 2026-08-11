import { useSyncState } from '@vautr/ui-logic';
import { RefreshCw } from 'lucide-react';

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
        <span className="size-2 rounded-full bg-emerald-400" aria-hidden="true" />
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
      <RefreshCw className="size-4 animate-spin" aria-hidden="true" />
      Syncing {sync.progress}%
    </span>
  );
}
