import type { VautrMlpClient } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { Inbox, Users } from 'lucide-react';
import { GroupsTab } from './GroupsTab';
import { InboxTab } from './InboxTab';

interface SharesTabProps {
  mlp: VautrMlpClient;
  client: VautrWebClient;
}

/**
 * Unified Shares surface (mirrors mobile's Shares screen and web's /shares
 * route): the user's incoming share Inbox plus their group sharing, presented
 * as two first-class sections under one tab. Decryption is local; the server
 * only stores ciphertext.
 */
export function SharesTab({ mlp, client }: SharesTabProps) {
  return (
    <div className="space-y-6">
      <section className="space-y-3">
        <div className="flex items-center gap-2">
          <Inbox className="size-4 text-accent" aria-hidden="true" />
          <h2 className="text-sm font-semibold text-text">Inbox</h2>
        </div>
        <InboxTab mlp={mlp} client={client} />
      </section>

      <section className="space-y-3 border-t border-border pt-4">
        <div className="flex items-center gap-2">
          <Users className="size-4 text-accent" aria-hidden="true" />
          <h2 className="text-sm font-semibold text-text">Groups</h2>
        </div>
        <GroupsTab client={client} mlp={mlp} />
      </section>
    </div>
  );
}
