import { createFileRoute } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { mlp, MlpApiError } from '@/lib/mlp';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { Badge } from '@/components/ui/badge';
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
import { Checkbox } from '@/components/ui/checkbox';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import type { AccessScope, MachineAccount } from '@vautr/api-contract';
import { Bot, MoreHorizontal, Plus, Trash2 } from 'lucide-react';

export const Route = createFileRoute('/_authed/machine-accounts')({
  component: MachineAccountsPage,
});

const SCOPES: AccessScope[] = [
  'secrets:read',
  'secrets:write',
  'secrets:reveal',
  'projects:read',
  'projects:write',
  'tokens:manage',
  'machine_accounts:manage',
];

function MachineAccountsPage() {
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
      setError(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

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
    if (!name.trim() || scopes.length === 0) {
      toast.error('Name and at least one scope are required');
      return;
    }
    setBusy(true);
    try {
      await mlp.createMachineAccount({ name: name.trim(), scopes });
      toast.success('Machine account created');
      setOpen(false);
      setName('');
      setScopes(['secrets:read']);
      void load();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  };

  const toggleStatus = (acc: MachineAccount) => {
    void mlp
      .updateMachineAccount(acc.uuid, { status: acc.status === 'active' ? 'disabled' : 'active' })
      .then(() => {
        toast.success(
          acc.status === 'active' ? 'Machine account disabled' : 'Machine account enabled',
        );
        void load();
      });
  };

  return (
    <div className="space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-text">Machine accounts</h1>
          <p className="text-sm text-text-muted">
            Non-human identities for CI/CD, apps, and agents.
          </p>
        </div>
        <Button onClick={() => setOpen(true)}>
          <Plus className="mr-1.5 size-4" aria-hidden="true" />
          New machine account
        </Button>
      </div>

      {error ? <p className="text-sm text-danger">{error}</p> : null}
      {loading ? <p className="text-sm text-text-muted">Loading…</p> : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Bot className="size-4 text-accent" aria-hidden="true" />
            Machine accounts
          </CardTitle>
        </CardHeader>
        <CardContent>
          {!loading && accounts.length === 0 ? (
            <p className="py-10 text-center text-sm text-text-muted">No machine accounts yet.</p>
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Status</TableHead>
                  <TableHead>Scopes</TableHead>
                  <TableHead>Created</TableHead>
                  <TableHead className="w-12" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {accounts.map((acc) => (
                  <TableRow key={acc.uuid}>
                    <TableCell className="text-text">
                      {acc.name}
                      <span className="block font-mono text-xs text-text-muted">{acc.uuid}</span>
                    </TableCell>
                    <TableCell>
                      <Badge variant={acc.status === 'active' ? 'default' : 'secondary'}>
                        {acc.status}
                      </Badge>
                    </TableCell>
                    <TableCell className="flex flex-wrap gap-1">
                      {acc.scopes.map((s) => (
                        <Badge key={s} variant="outline">
                          {s}
                        </Badge>
                      ))}
                    </TableCell>
                    <TableCell className="text-text-muted">
                      {new Date(acc.created_at).toLocaleDateString()}
                    </TableCell>
                    <TableCell>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="ghost" size="icon" aria-label="Machine account actions">
                            <MoreHorizontal className="size-4" aria-hidden="true" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem onClick={() => toggleStatus(acc)}>
                            {acc.status === 'active' ? 'Disable' : 'Enable'}
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            className="text-danger"
                            onClick={() => {
                              void mlp.deleteMachineAccount(acc.uuid).then(() => {
                                toast.success('Machine account deleted');
                                void load();
                              });
                            }}
                          >
                            <Trash2 className="mr-2 size-4" aria-hidden="true" />
                            Delete
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}
        </CardContent>
      </Card>

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New machine account</DialogTitle>
              <DialogDescription>Define the scopes this account may use.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="ma-name">Name</Label>
                <Input
                  id="ma-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="ci-deployer"
                />
              </div>
              <div className="space-y-1.5">
                <Label>Scopes</Label>
                <div className="grid gap-2">
                  {SCOPES.map((scope) => (
                    <label key={scope} className="flex items-center gap-2 text-sm text-text">
                      <Checkbox
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
