'use client';

import type { ConflictChoice } from '@vautr/client-sdk';
import { useConflictActions, useCurrentConflict } from '@vautr/ui-logic';
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
import { getClient } from '@/lib/client';

/**
 * Global conflict-resolution modal (VTR-056, data.md §7.2).
 *
 * Renders the head of the conflict queue (one at a time — TDD5: no overlap).
 * The store owns the queue; this component only reads the head and dispatches
 * the user's choice back through `VautrWebClient.resolveConflict`, which applies
 * it and refreshes the vault, after which the store dequeues the conflict.
 *
 * Accessibility: built on Radix `Dialog` (role="dialog", aria-modal, focus trap,
 * Esc-to-close). Closing without choosing dismisses the conflict (it stays
 * "ignored", TDD4) rather than resolving it.
 */
export function ConflictModal() {
  const conflict = useCurrentConflict();
  const { resolveConflict, dismissConflict } = useConflictActions();

  const onChoose = useCallback(
    async (uuid: string, choice: ConflictChoice) => {
      await getClient().resolveConflict(uuid, choice);
      resolveConflict(uuid);
    },
    [resolveConflict],
  );

  const onOpenChange = useCallback(
    (open: boolean) => {
      // Radix fires this with `open === false` on Escape / overlay click / X.
      // Treat any close-without-explicit-choice as a dismiss (TDD4).
      if (!open && conflict) {
        dismissConflict(conflict);
      }
    },
    [conflict, dismissConflict],
  );

  const open = conflict !== null;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent aria-describedby="conflict-desc">
        {conflict && (
          <>
            <DialogHeader>
              <DialogTitle id="conflict-title">
                {conflict.isToxic ? 'Unreadable update conflict' : 'Sync conflict'}
              </DialogTitle>
              <DialogDescription id="conflict-desc">
                {conflict.isToxic
                  ? 'Your edit conflicts with an unreadable update (possibly from a key rotation). Choose how to proceed.'
                  : 'This item was updated on another device. Choose which version to keep.'}
              </DialogDescription>
            </DialogHeader>

            <DialogFooter>
              {conflict.isToxic ? (
                <>
                  <Button
                    variant="outline"
                    onClick={() => void onChoose(conflict.uuid, 'acceptServer')}
                  >
                    Keep Local
                  </Button>
                  <Button
                    variant="default"
                    onClick={() => void onChoose(conflict.uuid, 'pushLocal')}
                  >
                    Overwrite Server
                  </Button>
                </>
              ) : (
                <>
                  <Button
                    variant="outline"
                    onClick={() => void onChoose(conflict.uuid, 'acceptServer')}
                  >
                    Keep Server Version
                  </Button>
                  <Button
                    variant="default"
                    onClick={() => void onChoose(conflict.uuid, 'pushLocal')}
                  >
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
