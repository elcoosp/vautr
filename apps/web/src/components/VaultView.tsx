import { useVaultActions } from '@vautr/ui-logic';
import { Lock, Vault } from 'lucide-react';
import { useState } from 'react';
import { ItemDetail } from './ItemDetail';
import { ReadOnlyBanner } from './ReadOnlyBanner';
import { SyncIndicator } from './SyncIndicator';
import { VaultList } from './VaultList';

export function VaultView() {
  const [selectedUuid, setSelectedUuid] = useState<string | null>(null);
  const { lock } = useVaultActions();

  return (
    <div className="flex h-screen flex-col bg-bg">
      <header className="flex items-center justify-between border-b border-border bg-surface px-4 py-3">
        <h1 className="flex items-center gap-2 text-lg font-semibold text-text">
          <Vault className="size-5 text-accent" aria-hidden="true" />
          Vault
        </h1>
        <div className="flex items-center gap-3">
          <SyncIndicator />
          <button
            type="button"
            onClick={lock}
            aria-label="Lock vault"
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm text-text-muted hover:bg-surface-raised hover:text-text"
          >
            <Lock className="size-4" aria-hidden="true" />
            Lock
          </button>
        </div>
      </header>

      <ReadOnlyBanner />

      <div className="flex min-h-0 flex-1">
        {/* List pane (hidden on small screens while an item is open). */}
        <section
          aria-label="Vault items"
          className={`w-full md:w-80 md:shrink-0 md:border-r md:border-border ${
            selectedUuid ? 'hidden md:block' : 'block'
          }`}
        >
          <VaultList selectedUuid={selectedUuid} onSelect={setSelectedUuid} />
        </section>

        {/* Detail pane. */}
        <section
          aria-label="Item details"
          className={`min-w-0 flex-1 ${selectedUuid ? 'block' : 'hidden md:block'}`}
        >
          {selectedUuid ? (
            <ItemDetail uuid={selectedUuid} onBack={() => setSelectedUuid(null)} />
          ) : (
            <p className="p-6 text-center text-sm text-text-muted">
              Select an item to view its details.
            </p>
          )}
        </section>
      </div>
    </div>
  );
}
