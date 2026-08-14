import { Inbox, Lock, Plus, Users, Vault } from 'lucide-react';
import { useState } from 'react';
import { lock } from '../lib/client';
import { AddItemForm } from './AddItemForm';
import { GroupsView } from './GroupsView';
import { InboxView } from './InboxView';
import { ItemDetail } from './ItemDetail';
import { ReadOnlyBanner } from './ReadOnlyBanner';
import { SyncIndicator } from './SyncIndicator';
import { VaultList } from './VaultList';

export function VaultView() {
  const [selectedUuid, setSelectedUuid] = useState<string | null>(null);
  const [adding, setAdding] = useState(false);
  const [tab, setTab] = useState<'vault' | 'inbox' | 'groups'>('vault');

  const onLock = () => {
    void lock();
  };

  return (
    <div className="flex h-screen flex-col bg-bg">
      <header className="flex items-center justify-between border-b border-border bg-surface px-4 py-3">
        <h1 className="flex items-center gap-2 text-lg font-semibold text-text">
          <Vault className="size-5 text-accent" aria-hidden="true" />
          Vault
        </h1>
        <div className="flex items-center gap-3">
          <div className="flex rounded-md border border-border">
            <button
              type="button"
              onClick={() => setTab('vault')}
              aria-pressed={tab === 'vault'}
              className={`px-3 py-1.5 text-sm ${
                tab === 'vault' ? 'bg-surface-raised text-text' : 'text-text-muted hover:text-text'
              }`}
            >
              <Vault className="mr-1 inline size-4" aria-hidden="true" />
              Vault
            </button>
            <button
              type="button"
              onClick={() => setTab('inbox')}
              aria-pressed={tab === 'inbox'}
              className={`px-3 py-1.5 text-sm ${
                tab === 'inbox' ? 'bg-surface-raised text-text' : 'text-text-muted hover:text-text'
              }`}
            >
              <Inbox className="mr-1 inline size-4" aria-hidden="true" />
              Inbox
            </button>
            <button
              type="button"
              onClick={() => setTab('groups')}
              aria-pressed={tab === 'groups'}
              className={`px-3 py-1.5 text-sm ${
                tab === 'groups' ? 'bg-surface-raised text-text' : 'text-text-muted hover:text-text'
              }`}
            >
              <Users className="mr-1 inline size-4" aria-hidden="true" />
              Groups
            </button>
          </div>
          <SyncIndicator />
          <button
            type="button"
            onClick={() => {
              setAdding(true);
              setSelectedUuid(null);
            }}
            aria-label="Add item"
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm text-text-muted hover:bg-surface-raised hover:text-text"
          >
            <Plus className="size-4" aria-hidden="true" />
            Add item
          </button>
          <button
            type="button"
            onClick={onLock}
            aria-label="Lock vault"
            className="inline-flex items-center gap-1.5 rounded-md border border-border px-3 py-1.5 text-sm text-text-muted hover:bg-surface-raised hover:text-text"
          >
            <Lock className="size-4" aria-hidden="true" />
            Lock
          </button>
        </div>
      </header>

      <ReadOnlyBanner />

      {tab === 'inbox' ? (
        <InboxView />
      ) : tab === 'groups' ? (
        <GroupsView />
      ) : (
        <div className="flex min-h-0 flex-1">
          {/* List pane (hidden on small screens while an item is open). */}
          <section
            aria-label="Vault items"
            className={`w-full md:w-80 md:shrink-0 md:border-r md:border-border ${
              selectedUuid || adding ? 'hidden md:block' : 'block'
            }`}
          >
            <VaultList selectedUuid={selectedUuid} onSelect={setSelectedUuid} />
          </section>

          {/* Detail pane. */}
          <section
            aria-label="Item details"
            className={`min-w-0 flex-1 ${selectedUuid || adding ? 'block' : 'hidden md:block'}`}
          >
            {adding ? (
              <AddItemForm
                onSaved={(uuid) => {
                  setAdding(false);
                  setSelectedUuid(uuid);
                }}
                onCancel={() => setAdding(false)}
              />
            ) : selectedUuid ? (
              <ItemDetail uuid={selectedUuid} onBack={() => setSelectedUuid(null)} />
            ) : (
              <p className="p-6 text-center text-sm text-text-muted">
                Select an item to view its details.
              </p>
            )}
          </section>
        </div>
      )}
    </div>
  );
}
