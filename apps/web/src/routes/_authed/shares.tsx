import { createFileRoute } from '@tanstack/react-router';
import { Inbox, Share2, Users } from 'lucide-react';
import { useState } from 'react';
import { GroupsView } from '@/components/GroupsView';
import { InboxView } from '@/components/InboxView';

export const Route = createFileRoute('/_authed/shares')({
  component: SharesPage,
});

type SharesTab = 'inbox' | 'groups';

function SharesPage() {
  const [tab, setTab] = useState<SharesTab>('inbox');

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="flex items-center gap-2 text-2xl font-semibold text-text">
          <Share2 className="size-5 text-accent" aria-hidden="true" />
          Shares
        </h1>
        <p className="text-sm text-text-muted">
          Secure sharing: items other users have encrypted for your sharing key, and the groups you
          belong to. Decryption always happens locally — the server only stores ciphertext.
        </p>
      </div>

      <div className="flex rounded-md border border-border p-1">
        <button
          type="button"
          onClick={() => setTab('inbox')}
          aria-pressed={tab === 'inbox'}
          className={`flex flex-1 items-center justify-center gap-1.5 rounded px-3 py-1.5 text-sm ${
            tab === 'inbox' ? 'bg-surface-raised text-text' : 'text-text-muted hover:text-text'
          }`}
        >
          <Inbox className="size-4" aria-hidden="true" />
          Inbox
        </button>
        <button
          type="button"
          onClick={() => setTab('groups')}
          aria-pressed={tab === 'groups'}
          className={`flex flex-1 items-center justify-center gap-1.5 rounded px-3 py-1.5 text-sm ${
            tab === 'groups' ? 'bg-surface-raised text-text' : 'text-text-muted hover:text-text'
          }`}
        >
          <Users className="size-4" aria-hidden="true" />
          Groups
        </button>
      </div>

      {tab === 'inbox' ? <InboxView /> : <GroupsView />}
    </div>
  );
}
