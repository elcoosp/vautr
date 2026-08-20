import { Inbox } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { EmptyState } from '@/components/EmptyState';
import { acceptShare, getShareInbox, revokeShare } from '../lib/client';

interface IncomingShare {
  share_id: string;
  sender_uuid: string;
  item_uuid: string;
  wrapped_sik: string;
  ephemeral_public_key: string;
  encrypted_payload: string | null;
}

/**
 * Shares waiting in the current user's inbox (ADR-007). Accepting decrypts the
 * item under the recipient's sharing key locally; the plaintext is shown
 * transiently and never persisted.
 */
export function InboxView() {
  const [shares, setShares] = useState<IncomingShare[]>([]);
  const [decrypted, setDecrypted] = useState<Record<string, string>>({});
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setError(null);
    try {
      const inbox = await getShareInbox();
      setShares(inbox);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load inbox.');
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleAccept(share: IncomingShare): Promise<void> {
    setBusy(share.share_id);
    setError(null);
    try {
      const plaintext = await acceptShare(share);
      const text = new TextDecoder().decode(plaintext);
      setDecrypted((prev) => ({ ...prev, [share.share_id]: text }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to decrypt share.');
    } finally {
      setBusy(null);
    }
  }

  async function handleRevoke(share: IncomingShare): Promise<void> {
    setBusy(share.share_id);
    setError(null);
    try {
      await revokeShare(share.item_uuid);
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to revoke share.');
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="flex h-full flex-col bg-bg">
      <header className="flex items-center justify-between border-b border-border px-4 py-3">
        <h1 className="text-lg font-semibold text-text">Shared with me</h1>
        <button
          type="button"
          onClick={() => void refresh()}
          className="rounded-md border border-border px-3 py-1.5 text-sm text-text-muted hover:bg-surface-raised hover:text-text"
        >
          Refresh
        </button>
      </header>
      <div className="flex-1 overflow-auto p-4">
        {error ? <p className="mb-3 text-sm text-destructive">{error}</p> : null}
        {shares.length === 0 ? (
          <EmptyState variant="inline" icon={Inbox} title="No pending shares." />
        ) : (
          <ul className="space-y-2">
            {shares.map((share) => (
              <li key={share.share_id} className="rounded-lg border border-border bg-surface p-4">
                <div className="flex items-center justify-between">
                  <span className="text-sm font-medium text-text">Item {share.item_uuid}</span>
                  <span className="text-xs text-text-muted">From {share.sender_uuid}</span>
                </div>
                {share.encrypted_payload ? (
                  <pre className="mt-2 max-h-40 overflow-auto rounded border border-border bg-bg p-2 text-xs text-text">
                    {decrypted[share.share_id] ?? share.encrypted_payload}
                  </pre>
                ) : null}
                <div className="mt-2 flex gap-2">
                  {decrypted[share.share_id] ? (
                    <span className="rounded-md border border-border px-3 py-1.5 text-sm text-text-muted">
                      Decrypted
                    </span>
                  ) : (
                    <button
                      type="button"
                      onClick={() => void handleAccept(share)}
                      disabled={busy === share.share_id}
                      className="rounded-md border border-border px-3 py-1.5 text-sm text-text hover:bg-surface-raised disabled:opacity-50"
                    >
                      {busy === share.share_id ? 'Decrypting…' : 'Accept & decrypt'}
                    </button>
                  )}
                  <button
                    type="button"
                    onClick={() => void handleRevoke(share)}
                    disabled={busy === share.share_id}
                    className="rounded-md px-3 py-1.5 text-sm text-destructive hover:bg-destructive/10 disabled:opacity-50"
                  >
                    Revoke
                  </button>
                </div>
              </li>
            ))}
          </ul>
        )}
      </div>
    </div>
  );
}
