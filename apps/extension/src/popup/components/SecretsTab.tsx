import { useEffect, useMemo, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { Separator } from '@/components/ui/separator';
import type { VautrMlpClient } from '@vautr/client-sdk';
import type { Secret } from '@vautr/api-contract';
import { usePopupStore } from '../store';

interface SecretsTabProps {
  mlp: VautrMlpClient;
}

/** Encode a plaintext secret value into the base64 envelope the server stores. */
function encodeValue(value: string): string {
  return btoa(value);
}

/** Decode a stored base64 envelope back to the plaintext value. */
function decodeValue(ciphertext: string): string {
  try {
    return atob(ciphertext);
  } catch {
    return ciphertext;
  }
}

export function SecretsTab({ mlp }: SecretsTabProps) {
  const projects = usePopupStore((s) => s.projects);
  const secrets = usePopupStore((s) => s.secrets);
  const setSecrets = usePopupStore((s) => s.setSecrets);
  const upsertSecret = usePopupStore((s) => s.upsertSecret);
  const removeSecret = usePopupStore((s) => s.removeSecret);
  const setError = usePopupStore((s) => s.setError);

  const [projectUuid, setProjectUuid] = useState<string>('__none__');
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [denied, setDenied] = useState<Record<string, string>>({});

  const [showCreate, setShowCreate] = useState(false);
  const [key, setKey] = useState('');
  const [value, setValue] = useState('');

  const selectedProject = useMemo(
    () => projects.find((p) => p.uuid === projectUuid),
    [projects, projectUuid],
  );

  async function refreshSecrets(): Promise<void> {
    if (projectUuid === '__none__') {
      setSecrets([]);
      return;
    }
    try {
      const res = await mlp.listSecrets(projectUuid);
      setSecrets(res.secrets);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  useEffect(() => {
    void refreshSecrets();
  }, [projectUuid]);

  async function handleCreate(): Promise<void> {
    if (projectUuid === '__none__' || !key || !value) {
      setError('Choose a project and enter a key + value.');
      return;
    }
    try {
      const secret = await mlp.createSecret({
        project_uuid: projectUuid,
        key,
        value_ciphertext: encodeValue(value),
      });
      upsertSecret(secret);
      setKey('');
      setValue('');
      setShowCreate(false);
      toast.success(`Created secret "${secret.key}".`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleReveal(secret: Secret): Promise<void> {
    if (revealed[secret.uuid]) {
      setRevealed((prev) => {
        const next = { ...prev };
        delete next[secret.uuid];
        return next;
      });
      setDenied((prev) => {
        const next = { ...prev };
        delete next[secret.uuid];
        return next;
      });
      return;
    }
    setDenied((prev) => {
      const next = { ...prev };
      delete next[secret.uuid];
      return next;
    });
    try {
      const res = await mlp.getSecretValue(secret.uuid);
      setRevealed((prev) => ({ ...prev, [secret.uuid]: decodeValue(res.value_ciphertext) }));
      toast.success(`Revealed "${secret.key}".`);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setDenied((prev) => ({ ...prev, [secret.uuid]: message }));
      setRevealed((prev) => {
        const next = { ...prev };
        delete next[secret.uuid];
        return next;
      });
      toast.error(`Reveal denied: ${message}`);
    }
  }

  return (
    <div className="space-y-4">
      <div className="space-y-1">
        <Label>Project</Label>
        <Select value={projectUuid} onValueChange={(v) => setProjectUuid(v ?? '')}>
          <SelectTrigger className="w-full">
            <SelectValue placeholder="Select a project" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="__none__" disabled>
              Select a project…
            </SelectItem>
            {projects.map((p) => (
              <SelectItem key={p.uuid} value={p.uuid}>
                {p.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </div>

      {selectedProject ? (
        <>
          <div className="flex items-center justify-between">
            <div>
              <h3 className="text-sm font-semibold">Secrets</h3>
              <p className="text-xs text-muted-foreground">
                {secrets.length} in “{selectedProject.name}”
              </p>
            </div>
            <Button size="sm" onClick={() => setShowCreate(true)}>
              New secret
            </Button>
          </div>

          {secrets.length === 0 ? (
            <p className="text-sm text-muted-foreground">No secrets in this project yet.</p>
          ) : (
            <div className="space-y-2">
              {secrets.map((s) => {
                const valueText = revealed[s.uuid];
                const denial = denied[s.uuid];
                return (
                  <div key={s.uuid} className="rounded-lg border p-3">
                    <div className="flex items-center justify-between">
                      <div className="min-w-0">
                        <span className="text-sm font-medium">{s.key}</span>
                        <Badge variant="outline" className="ml-2">
                          v{s.version}
                        </Badge>
                      </div>
                      <Button size="sm" variant="outline" onClick={() => void handleReveal(s)}>
                        {valueText || denial ? 'Hide' : 'Reveal'}
                      </Button>
                    </div>
                    {valueText ? (
                      <div className="mt-2 rounded border bg-muted/40 px-2 py-1 font-mono text-xs break-all">
                        {valueText}
                      </div>
                    ) : null}
                    {denial ? (
                      <div className="mt-2 rounded border border-destructive/40 bg-destructive/10 px-2 py-1 text-xs text-destructive">
                        Reveal denied: {denial}
                      </div>
                    ) : null}
                  </div>
                );
              })}
            </div>
          )}
        </>
      ) : (
        <p className="text-sm text-muted-foreground">Select a project to manage its secrets.</p>
      )}

      <Dialog open={showCreate} onOpenChange={setShowCreate}>
        <DialogContent>
          <DialogHeader>
            <DialogTitle>New secret</DialogTitle>
            <DialogDescription>Add a key/value secret to this project.</DialogDescription>
          </DialogHeader>
          <div className="space-y-3">
            <div className="space-y-1">
              <Label>Key</Label>
              <Input value={key} onChange={(e) => setKey(e.target.value)} />
            </div>
            <div className="space-y-1">
              <Label>Value</Label>
              <Input value={value} onChange={(e) => setValue(e.target.value)} />
            </div>
          </div>
          <DialogFooter>
            <Button onClick={() => void handleCreate()}>Save</Button>
          </DialogFooter>
          <Separator />
        </DialogContent>
      </Dialog>
    </div>
  );
}
