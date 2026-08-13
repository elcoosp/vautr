'use client';

import type { ConflictChoice, ConflictEvent } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { useCallback } from 'react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { usePopupStore } from '../store';

/**
 * Conflict-resolution modal (VTR-064 / VTR-056).
 *
 * Mirrors the web `ConflictModal`: renders the head of the FIFO conflict queue
 * (one at a time — no overlap). The store owns the queue; this component reads
 * the head and dispatches the user's choice through the client's
 * `resolveConflict`, then dequeues. Closing without choosing dismisses.
 */
export function ConflictModal({ client }: { client: VautrWebClient }) {
  const conflict = usePopupStore((s) => s.conflictQueue[0] ?? null);
  const dismissConflict = usePopupStore((s) => s.dismissConflict);

  const onChoose = useCallback(
    async (event: ConflictEvent, choice: ConflictChoice) => {
      await client.resolveConflict(event.uuid, choice);
      dismissConflict(event.uuid);
    },
    [client, dismissConflict],
  );

  const onOpenChange = useCallback(
    (open: boolean) => {
      if (!open && conflict) dismissConflict(conflict.uuid);
    },
    [conflict, dismissConflict],
  );

  const open = conflict !== null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby="ext-conflict-desc">
        {conflict && (
          <>
            <DialogHeader>
              <DialogTitle id="ext-conflict-title">
                {conflict.isToxic ? 'Unreadable update conflict' : 'Sync conflict'}
              </DialogTitle>
              <DialogDescription id="ext-conflict-desc">
                {conflict.isToxic
                  ? 'Your edit conflicts with an unreadable update (possibly from a key rotation). Choose how to proceed.'
                  : 'This item was updated on another device. Choose which version to keep.'}
              </DialogDescription>
            </DialogHeader>

            <DialogFooter>
              {conflict.isToxic ? (
                <>
                  <Button variant="outline" onClick={() => void onChoose(conflict, 'acceptServer')}>
                    Keep Local
                  </Button>
                  <Button variant="default" onClick={() => void onChoose(conflict, 'pushLocal')}>
                    Overwrite Server
                  </Button>
                </>
              ) : (
                <>
                  <Button variant="outline" onClick={() => void onChoose(conflict, 'acceptServer')}>
                    Keep Server Version
                  </Button>
                  <Button variant="default" onClick={() => void onChoose(conflict, 'pushLocal')}>
                    Force Overwrite with Local
                  </Button>
                </>
              )}
            </DialogFooter>
          </>
        )}
      </DialogContent>
    </Dialog>
  );
}
