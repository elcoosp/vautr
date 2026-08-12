import { useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
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
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { assessPassword, isReusedPassword, strengthLabel } from '@vautr/client-sdk';
import { usePopupStore } from '../store';

interface VaultTabProps {
  client: VautrWebClient;
}

export function VaultTab({ client }: VaultTabProps) {
  const items = usePopupStore((s) => s.items);
  const revealed = usePopupStore((s) => s.revealedPasswords);
  const addRevealedPassword = usePopupStore((s) => s.addRevealedPassword);
  const setError = usePopupStore((s) => s.setError);
  const setStatus = usePopupStore((s) => s.setStatus);

  const [showAdd, setShowAdd] = useState(false);
  const [addTitle, setAddTitle] = useState('');
  const [addUser, setAddUser] = useState('');
  const [addPass, setAddPass] = useState('');
  const [addUrl, setAddUrl] = useState('');
  const [revealedSecrets, setRevealedSecrets] = useState<Record<string, string>>({});

  const assessment = addPass ? assessPassword(addPass) : null;
  const reused = addPass ? isReusedPassword(addPass, revealed) : false;

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
      addRevealedPassword(addPass);
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

  async function handleAutofill(uuid: string, title: string): Promise<void> {
    try {
      const { autofillItem } = await import('../vaultActions');
      const res = await autofillItem(client, uuid);
      if (res.ok) toast.success(res.message);
      else toast.error(res.message);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

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

  async function handleReveal(item: {
    uuid: string;
    title: string;
    subtitle: string;
  }): Promise<void> {
    try {
      if (revealedSecrets[item.uuid]) {
        setRevealedSecrets((prev) => {
          const next = { ...prev };
          delete next[item.uuid];
          return next;
        });
        return;
      }
      const { revealSecret } = await import('../vaultActions');
      const secret = await revealSecret(client, item.uuid);
      addRevealedPassword(secret);
      setRevealedSecrets((prev) => ({ ...prev, [item.uuid]: secret }));
      toast.success(`Revealed "${item.title}".`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
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
        <Button size="sm" onClick={() => setShowAdd(true)}>
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
            const revealedSecret = revealedSecrets[item.uuid];
            return (
              <Card key={item.uuid}>
                <CardHeader className="space-y-0 py-3">
                  <CardTitle className="text-sm">{item.title}</CardTitle>
                  <CardDescription className="text-xs">{item.subtitle}</CardDescription>
                </CardHeader>
                <CardContent className="space-y-2 py-2">
                  {revealedSecret ? (
                    <div className="rounded border bg-muted/40 px-2 py-1 font-mono text-xs break-all">
                      {revealedSecret}
                    </div>
                  ) : null}
                  <div className="flex gap-2">
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() => void handleAutofill(item.uuid, item.title)}
                    >
                      Autofill
                    </Button>
                    <Button size="sm" variant="outline" onClick={() => void handleReveal(item)}>
                      {revealedSecret ? 'Hide' : 'Reveal'}
                    </Button>
                    <Button size="sm" variant="ghost" onClick={() => void handleCopy(item)}>
                      Copy
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
              {assessment ? (
                <div className="flex items-center gap-2 pt-1">
                  <Badge
                    variant={
                      assessment.score <= 1
                        ? 'destructive'
                        : assessment.score === 2
                          ? 'secondary'
                          : 'default'
                    }
                  >
                    {strengthLabel(assessment.score)} · {assessment.entropy} bits
                  </Badge>
                  {reused ? (
                    <Badge variant="destructive">Reused</Badge>
                  ) : (
                    <Badge variant="outline">New</Badge>
                  )}
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
    </div>
  );
}
