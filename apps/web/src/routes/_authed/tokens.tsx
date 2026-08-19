import { createFileRoute } from '@tanstack/react-router';
import type { AccessScope, AccessToken } from '@vautr/api-contract';
import { Skeleton } from 'boneyard-js/react';
import { Copy, MoreHorizontal, Plus, Ticket, Trash2 } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { EmptyState } from '@/components/EmptyState';
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from '@/components/ui/table';
import { MlpApiError, mlp } from '@/lib/mlp';

export const Route = createFileRoute('/_authed/tokens')({
  component: TokensPage,
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

function TokensPage() {
  const [tokens, setTokens] = useState<AccessToken[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState(false);
  const [name, setName] = useState('');
  const [scopes, setScopes] = useState<AccessScope[]>(['secrets:read']);
  const [busy, setBusy] = useState(false);
  const [issuedSecret, setIssuedSecret] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await mlp.listTokens();
      setTokens(res.tokens);
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
    if (!name.trim() || scopes.length === 0) return;
    setBusy(true);
    try {
      const res = await mlp.createToken({ name: name.trim(), scopes });
      setIssuedSecret(res.token);
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

  return (
    <div className="space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-semibold text-text">Access tokens</h1>
          <p className="text-sm text-text-muted">
            Issue scoped tokens with expiration and revocation.
          </p>
        </div>
        <Button onClick={() => setOpen(true)}>
          <Plus className="mr-1.5 size-4" aria-hidden="true" />
          New token
        </Button>
      </div>

      {error ? <p className="text-sm text-danger">{error}</p> : null}
      {loading ? (
        <Skeleton
          name="tokens-loading"
          loading
          fallback={<p className="text-sm text-text-muted">Loading…</p>}
        >
          {null}
        </Skeleton>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Ticket className="size-4 text-accent" aria-hidden="true" />
            Tokens
          </CardTitle>
        </CardHeader>
        <CardContent>
          {!loading && tokens.length === 0 ? (
            <EmptyState variant="inline" icon={Ticket} title="No access tokens yet." />
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Name</TableHead>
                  <TableHead>Prefix</TableHead>
                  <TableHead>Scopes</TableHead>
                  <TableHead>Expires</TableHead>
                  <TableHead className="w-12" />
                </TableRow>
              </TableHeader>
              <TableBody>
                {tokens.map((t) => (
                  <TableRow key={t.uuid}>
                    <TableCell className="text-text">
                      {t.name}
                      <span className="block font-mono text-xs text-text-muted">{t.uuid}</span>
                    </TableCell>
                    <TableCell className="font-mono text-text-muted">{t.prefix}…</TableCell>
                    <TableCell className="flex flex-wrap gap-1">
                      {t.scopes.map((s) => (
                        <Badge key={s} variant="outline">
                          {s}
                        </Badge>
                      ))}
                    </TableCell>
                    <TableCell className="text-text-muted">
                      {t.expires_at ? new Date(t.expires_at).toLocaleDateString() : 'never'}
                    </TableCell>
                    <TableCell>
                      <DropdownMenu>
                        <DropdownMenuTrigger asChild>
                          <Button variant="ghost" size="icon" aria-label="Token actions">
                            <MoreHorizontal className="size-4" aria-hidden="true" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end">
                          <DropdownMenuItem
                            className="text-danger"
                            onClick={() => {
                              void mlp.revokeToken(t.uuid).then(() => {
                                toast.success('Token revoked');
                                void load();
                              });
                            }}
                          >
                            <Trash2 className="mr-2 size-4" aria-hidden="true" />
                            Revoke
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

      {issuedSecret ? (
        <Card>
          <CardHeader>
            <CardTitle className="text-warn">Save this token now</CardTitle>
          </CardHeader>
          <CardContent className="space-y-3">
            <p className="text-sm text-text-muted">
              The full token is shown only once. Store it somewhere safe.
            </p>
            <div className="flex gap-2">
              <code className="flex-1 truncate rounded-md border border-border bg-surface-raised px-3 py-2 font-mono text-sm text-text">
                {issuedSecret}
              </code>
              <Button
                variant="outline"
                onClick={() => {
                  void navigator.clipboard
                    .writeText(issuedSecret)
                    .then(() => toast.success('Copied'));
                }}
              >
                <Copy className="mr-1.5 size-4" aria-hidden="true" />
                Copy
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent className="sm:max-w-md">
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New access token</DialogTitle>
              <DialogDescription>Issue a token with fine-grained scopes.</DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="token-name">Name</Label>
                <Input
                  id="token-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="ci-token"
                />
              </div>
              <div className="space-y-1.5">
                <Label>Scopes</Label>
                <div className="grid gap-2">
                  {SCOPES.map((scope) => (
                    <label
                      key={scope}
                      htmlFor={`tok-scope-${scope}`}
                      className="flex items-center gap-2 text-sm text-text"
                    >
                      <Checkbox
                        id={`tok-scope-${scope}`}
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
                {busy ? 'Creating…' : 'Create token'}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
