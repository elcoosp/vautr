import type { AccessScope, MachineAccount } from '@vautr/api-contract';
import type { VautrMlpClient } from '@vautr/client-sdk';
import { Skeleton } from 'boneyard-js/react';
import { Bot, MoreHorizontal, Plus, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { EmptyState } from '@/popup/components/EmptyState';

const SCOPES: AccessScope[] = [
  'secrets:read',
  'secrets:write',
  'secrets:reveal',
  'projects:read',
  'projects:write',
  'tokens:manage',
  'machine_accounts:manage',
];

/**
 * Machine accounts surface (VTR-064 / VTR-047). Mirrors the web
 * `machine-accounts` route, adapted to the extension popup.
 */
export function MachineAccountsTab({ mlp }: { mlp: VautrMlpClient }) {
  const [accounts, setAccounts] = useState<MachineAccount[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [scopes, setScopes] = useState<AccessScope[]>(['secrets:read']);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const res = await mlp.listMachineAccounts();
      setAccounts(res.machine_accounts);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, [mlp]);

  useEffect(() => {
    void load();
  }, [load]);

  const toggleScope = (scope: AccessScope) => {
    setScopes((prev) =>
      prev.includes(scope) ? prev.filter((s) => s !== scope) : [...prev, scope],
    );
  };

  const onCreate = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!name.trim() || scopes.length === 0) return;
    setBusy(true);
    try {
      await mlp.createMachineAccount({ name: name.trim(), scopes });
      setOpen(false);
      setName('');
      setScopes(['secrets:read']);
      void load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const toggleStatus = (acc: MachineAccount) => {
    void mlp
      .updateMachineAccount(acc.uuid, { status: acc.status === 'active' ? 'disabled' : 'active' })
      .then(() => load())
      .catch((err) => setError(err instanceof Error ? err.message : String(err)));
  };

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-base font-semibold text-text">
            <Bot className="size-4 text-accent" aria-hidden="true" />
            Machine accounts
          </h2>
          <p className="text-xs text-text-muted">Non-human identities for CI/CD and agents.</p>
        </div>
        <Button size="sm" onClick={() => setOpen(true)}>
          <Plus className="mr-1 size-4" aria-hidden="true" /> New
        </Button>
      </div>

      {error ? <p className="text-xs text-destructive">{error}</p> : null}
      {loading ? (
        <Skeleton
          name="machine-accounts-tab-loading"
          loading
          fallback={<p className="text-xs text-text-muted">Loading…</p>}
        >
          {null}
        </Skeleton>
      ) : null}

      <Card size="sm">
        <CardHeader>
          <CardTitle className="text-sm">Machine accounts</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          {!loading && accounts.length === 0 ? (
            <EmptyState variant="inline" icon={Bot} title="No machine accounts yet." />
          ) : (
            accounts.map((acc) => (
              <div
                key={acc.uuid}
                className="flex items-center justify-between rounded-md border border-border px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm text-text">{acc.name}</div>
                  <div className="flex flex-wrap gap-1">
                    {acc.scopes.map((s) => (
                      <Badge key={s} variant="outline" className="text-[10px]">
                        {s}
                      </Badge>
                    ))}
                  </div>
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger
                    render={
                      <Button variant="ghost" size="icon" aria-label="Machine account actions">
                        <MoreHorizontal className="size-4" aria-hidden="true" />
                      </Button>
                    }
                  />
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem onClick={() => toggleStatus(acc)}>
                      {acc.status === 'active' ? 'Disable' : 'Enable'}
                    </DropdownMenuItem>
                    <DropdownMenuItem
                      className="text-destructive"
                      onClick={() => {
                        void mlp
                          .deleteMachineAccount(acc.uuid)
                          .then(() => load())
                          .catch((err) =>
                            setError(err instanceof Error ? err.message : String(err)),
                          );
                      }}
                    >
                      <Trash2 className="mr-2 size-4" aria-hidden="true" /> Delete
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New machine account</DialogTitle>
              <DialogDescription className="text-xs">
                Define the scopes this account may use.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="ext-ma-name">Name</Label>
                <Input
                  id="ext-ma-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="ci-deployer"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">Scopes</Label>
                <div className="grid gap-2">
                  {SCOPES.map((scope) => (
                    <label
                      key={scope}
                      htmlFor={`ext-ma-scope-${scope}`}
                      className="flex items-center gap-2 text-xs text-text"
                    >
                      <Checkbox
                        id={`ext-ma-scope-${scope}`}
                        checked={scopes.includes(scope)}
                        onCheckedChange={() => toggleScope(scope)}
                      />
                      <code className="rounded bg-surface-raised px-1">{scope}</code>
                    </label>
                  ))}
                </div>
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy}>
                {busy ? 'Creating…' : 'Create'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
