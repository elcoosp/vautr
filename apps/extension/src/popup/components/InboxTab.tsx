import type { VautrMlpClient } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Label } from '@/components/ui/label';

interface InboxTabProps {
  mlp: VautrMlpClient;
  client: VautrWebClient;
}

interface IncomingShare {
  share_id: string;
  sender_uuid: string;
  item_uuid: string;
  wrapped_sik: string;
  ephemeral_public_key: string;
  payload: string | null;
}

export function InboxTab({ mlp, client }: InboxTabProps) {
  const [shares, setShares] = useState<IncomingShare[]>([]);
  const [error, setError] = useState('');
  const [busy, setBusy] = useState<string | null>(null);

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const inbox = await client.getShareInbox(mlp);
      setShares(inbox);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [client, mlp]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleAccept(share: IncomingShare): Promise<void> {
    setBusy(share.share_id);
    setError('');
    try {
      const plaintext = await client.acceptShare(mlp, share);
      const text = new TextDecoder().decode(plaintext);
      toast.success(`Received “${share.item_uuid}” from ${share.sender_uuid}.`);
      // The plaintext is shown transiently; never persisted in React state beyond
      // this render. The recipient can copy it from here.
      setShares((prev) =>
        prev.map((s) => (s.share_id === share.share_id ? { ...s, payload: text } : s)),
      );
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }

  async function handleRevoke(share: IncomingShare): Promise<void> {
    setBusy(share.share_id);
    setError('');
    try {
      await client.revokeShare(mlp, share.item_uuid);
      toast.success('Share revoked.');
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Shared with me</h3>
          <p className="text-xs text-muted-foreground">
            Items other users have encrypted for your sharing key.
          </p>
        </div>
        <Button size="sm" variant="outline" onClick={() => void refresh()}>
          Refresh
        </Button>
      </div>

      {error ? <p className="text-sm text-destructive">{error}</p> : null}

      {shares.length === 0 ? (
        <p className="text-sm text-muted-foreground">No pending shares.</p>
      ) : (
        <div className="space-y-2">
          {shares.map((share) => (
            <Card key={share.share_id}>
              <CardHeader className="space-y-0 py-3">
                <CardTitle className="text-sm">Item {share.item_uuid}</CardTitle>
                <CardDescription className="text-xs">From {share.sender_uuid}</CardDescription>
              </CardHeader>
              <CardContent className="space-y-2 py-2">
                {share.payload ? (
                  <div className="space-y-1">
                    <Label>Decrypted content</Label>
                    <pre className="max-h-40 overflow-auto rounded border bg-muted p-2 text-xs">
                      {share.payload}
                    </pre>
                  </div>
                ) : null}
                <div className="flex gap-2">
                  {share.payload ? (
                    <Badge variant="secondary">Decrypted</Badge>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={busy === share.share_id}
                      onClick={() => void handleAccept(share)}
                    >
                      {busy === share.share_id ? 'Decrypting…' : 'Accept & decrypt'}
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    disabled={busy === share.share_id}
                    onClick={() => void handleRevoke(share)}
                  >
                    Revoke
                  </Button>
                </div>
              </CardContent>
            </Card>
          ))}
        </div>
      )}
    </div>
  );
}
