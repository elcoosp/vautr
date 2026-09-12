import type { VautrMlpClient } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { evaluatePasswordStrength } from '@vautr/ui-logic';
import { useState } from 'react';
import { toast } from 'sonner';
import { Favicon } from '@/components/Favicon';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { usePopupStore } from '../store';

interface VaultTabProps {
  client: VautrWebClient;
  mlp: VautrMlpClient;
}

export function VaultTab({ client, mlp }: VaultTabProps) {
  const items = usePopupStore((s) => s.items);
  const setError = usePopupStore((s) => s.setError);
  const setStatus = usePopupStore((s) => s.setStatus);

  const [showAdd, setShowAdd] = useState(false);
  const [addTitle, setAddTitle] = useState('');
  const [addUser, setAddUser] = useState('');
  const [addPass, setAddPass] = useState('');
  const [addUrl, setAddUrl] = useState('');

  const strength = addPass ? evaluatePasswordStrength(addPass) : null;

  async function handleAdd(): Promise<void> {
    if (!addTitle || !addUser || !addPass) {
      setError('Title, username and password are required.');
      return;
    }
    setStatus('busy');
    setError(null);
    try {
      await client.addItem({
        title: addTitle,
        username: addUser,
        password: addPass,
        url: addUrl,
      });
      await client.sync();
      setAddTitle('');
      setAddUser('');
      setAddPass('');
      setAddUrl('');
      setShowAdd(false);
      toast.success('Item added.');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setStatus('unlocked');
    }
  }

  async function handleAutofill(uuid: string, _title: string): Promise<void> {
    try {
      const { autofillItem } = await import('../vaultActions');
      const res = await autofillItem(client, uuid);
      if (res.ok) toast.success(res.message);
      else toast.error(res.message);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  /** ZK reveal: copy via an opaque wasm handle; plaintext never enters React state. */
  async function handleCopy(item: { uuid: string; title: string }): Promise<void> {
    try {
      const { copySecret } = await import('../vaultActions');
      await copySecret(
        client,
        items.find((i) => i.uuid === item.uuid) ?? {
          uuid: item.uuid,
          title: item.title,
          subtitle: '',
          iconKey: 'key',
          urls: [],
          updatedAt: 0,
        },
      );
      toast.success(`Copied "${item.title}".`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  // --- sharing (ADR-007) ---
  const [shareItem, setShareItem] = useState<{ uuid: string; title: string } | null>(null);
  const [shareRecipient, setShareRecipient] = useState('');
  const [shareBusy, setShareBusy] = useState(false);

  async function handleShare(item: { uuid: string; title: string }): Promise<void> {
    setShareItem(item);
    setShareRecipient('');
  }

  async function confirmShare(): Promise<void> {
    if (!shareItem || !shareRecipient) {
      setError('Recipient user id is required.');
      return;
    }
    setShareBusy(true);
    setError(null);
    try {
      const plaintext = await client.ensureSharingKey(mlp).then(async () => {
        return client.getItemPlaintext(shareItem.uuid);
      });
      await client.shareItem(mlp, shareItem.uuid, shareRecipient, plaintext);
      toast.success(`Shared "${shareItem.title}" with ${shareRecipient}.`);
      setShareItem(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setShareBusy(false);
    }
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="text-sm font-semibold">Vault</h3>
          <p className="text-xs text-muted-foreground">
            {items.length} saved login{items.length === 1 ? '' : 's'}
          </p>
        </div>
        <Button size="sm" data-tour="add-secret" onClick={() => setShowAdd(true)}>
          + Add
        </Button>
      </div>

      {items.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          No logins yet. Add one, or sync to pull from the server.
        </p>
      ) : (
        <div className="space-y-2">
          {items.map((item) => {
            return (
              <Card key={item.uuid}>
                <CardHeader className="space-y-0 py-3">
                  <div className="flex items-center gap-2">
                    <Favicon url={item.urls[0]} size={20} className="shrink-0" />
                    <CardTitle className="text-sm">{item.title}</CardTitle>
                  </div>
                  <CardDescription className="text-xs">{item.subtitle}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-2 py-2">
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void handleAutofill(item.uuid, item.title)}
                    >
                      Autofill
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => void handleCopy(item)}>
                      Copy
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => void handleShare(item)}>
                      Share
                    </Button>
                  </div>
                </CardContent>
              </Card>
            );
          })}
        </div>
      )}

      <Dialog open={showAdd} onOpenChange={setShowAdd}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Add login</DialogTitle>
            <DialogDescription>Save a new username/password pair to your vault.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label>Title</Label>
              <Input value={addTitle} onChange={(e) => setAddTitle(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>Username / email</Label>
              <Input value={addUser} onChange={(e) => setAddUser(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>Password</Label>
              <Input type="password" value={addPass} onChange={(e) => setAddPass(e.target.value)} />
              {strength ? (
                <div className="mt-1.5 space-y-1">
                  <div className="flex items-center gap-2">
                    <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-border">
                      <div
                        className={`h-full transition-all ${strength.score <= 1 ? 'bg-danger' : strength.score <= 3 ? 'bg-danger' : strength.score <= 4 ? 'bg-warn' : 'bg-accent'}`}
                        style={{ width: `${(strength.score / 7) * 100}%` }}
                      />
                    </div>
                    <span className={`text-xs font-medium ${strength.color}`}>
                      {strength.label}
                    </span>
                  </div>
                  {strength.suggestions.length > 0 ? (
                    <p className="text-xs text-muted-foreground">{strength.suggestions[0]}</p>
                  ) : null}
                </div>
              ) : null}
            </div>
            <div className="space-y-1">
              <Label>URL (optional)</Label>
              <Input value={addUrl} onChange={(e) => setAddUrl(e.target.value)} />
            </div>
          </div>
          <DialogFooter>
            <Button onClick={() => void handleAdd()}>Save</Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      <Dialog open={shareItem !== null} onOpenChange={(o) => !o && setShareItem(null)}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>Share “{shareItem?.title}”</DialogTitle>
            <DialogDescription>
              Encrypts the item under a one-time key and delivers it to the recipient’s inbox. The
              server only ever stores ciphertext.
            </DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label>Recipient user id</Label>
              <Input
                value={shareRecipient}
                onChange={(e) => setShareRecipient(e.target.value)}
                placeholder="recipient username"
              />
            </div>
          </div>
          <DialogFooter>
            <Button onClick={() => void confirmShare()} disabled={shareBusy}>
              {shareBusy ? 'Sharing…' : 'Share'}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
}
