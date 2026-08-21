import type { Secret } from '@vautr/api-contract';
import type { VautrMlpClient } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { KeyRound } from 'lucide-react';
import { useCallback, useEffect, useMemo, useState } from 'react';
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
import { EmptyState } from '@/popup/components/EmptyState';
import { usePopupStore } from '../store';

interface SecretsTabProps {
  mlp: VautrMlpClient;
  client: VautrWebClient;
}

export function SecretsTab({ mlp, client }: SecretsTabProps) {
  const projects = usePopupStore((s) => s.projects);
  const secrets = usePopupStore((s) => s.secrets);
  const setSecrets = usePopupStore((s) => s.setSecrets);
  const upsertSecret = usePopupStore((s) => s.upsertSecret);
  const _removeSecret = usePopupStore((s) => s.removeSecret);
  const setError = usePopupStore((s) => s.setError);

  const [projectUuid, setProjectUuid] = useState<string>('__none__');
  const [showCreate] = useState(false);
  const [key, setKey] = useState('');
  const [value, setValue] = useState('');
  const [query, setQuery] = useState('');

  const visibleSecrets = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q === '' ? secrets : secrets.filter((s) => s.key.toLowerCase().includes(q));
  }, [secrets, query]);

  const selectedProject = useMemo(
    () => projects.find((p) => p.uuid === projectUuid),
    [projects, projectUuid],
  );

  const refreshSecrets = useCallback(async (): Promise<void> => {
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
  }, [mlp, projectUuid, setSecrets, setError]);

  useEffect(() => {
    void refreshSecrets();
  }, [refreshSecrets]);

  async function handleCreate(): Promise<void> {
    if (projectUuid === '__none__' || !key || !value) {
      setError('Choose a project and enter a key + value.');
      return;
    }
    try {
      const value_ciphertext = await client.encryptSecretValue(projectUuid, value);
      const secret = await mlp.createSecret({
        project_uuid: projectUuid,
        key,
        value_ciphertext,
      });
      upsertSecret(secret);
      setKey('');
      setValue('');
      toast.success(`Created secret "${secret.key}".`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  /**
   * ZK reveal (VTR-062 follow-up): decrypt the secret value in wasm, copy it to
   * the clipboard, then drop the plaintext. The value is never stored in React
   * state or rendered into the DOM.
   */
  async function handleReveal(secret: Secret): Promise<void> {
    try {
      const res = await mlp.getSecretValue(secret.uuid);
      const plaintext = await client.decryptSecretValue(projectUuid, res.value_ciphertext);
      await navigator.clipboard.writeText(plaintext);
      toast.success(`Copied "${secret.key}".`);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
      toast.error(`Reveal failed: ${message}`);
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
            <Button size="sm" onClick={() => void handleCreate()}>
              New secret
            </Button>
          </div>

          <Input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Search secrets…"
            aria-label="Search secrets"
          />

          {secrets.length === 0 ? (
            <EmptyState variant="inline" icon={KeyRound} title="No secrets in this project yet." />
          ) : visibleSecrets.length === 0 ? (
            <p className="text-xs text-muted-foreground">No secrets match “{query}”.</p>
          ) : (
            <div className="space-y-2">
              {visibleSecrets.map((s) => (
                <div key={s.uuid} className="rounded-lg border p-3">
                  <div className="flex items-center justify-between">
                    <div className="min-w-0">
                      <span className="text-sm font-medium">{s.key}</span>
                      <Badge variant="outline" className="ml-2">
                        v{s.version}
                      </Badge>
                    </div>
                    <Button size="sm" variant="outline" onClick={() => void handleReveal(s)}>
                      Copy
                    </Button>
                  </div>
                </div>
              ))}
            </div>
          )}
        </>
      ) : (
        <p className="text-sm text-muted-foreground">Select a project to manage its secrets.</p>
      )}

      <Dialog open={showCreate} onOpenChange={() => undefined}>
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
