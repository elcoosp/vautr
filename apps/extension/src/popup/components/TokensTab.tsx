import type { AccessScope, AccessToken } from '@vautr/api-contract';
import type { VautrMlpClient } from '@vautr/client-sdk';
import { Skeleton } from 'boneyard-js/react';
import { Copy, MoreHorizontal, Plus, Ticket, Trash2 } from 'lucide-react';
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
 * Access tokens surface (VTR-064 / VTR-047). Mirrors the web `tokens` route,
 * adapted to the extension popup.
 */
export function TokensTab({ mlp }: { mlp: VautrMlpClient }) {
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
      const res = await mlp.createToken({ name: name.trim(), scopes });
      setIssuedSecret(res.token);
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

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="flex items-center gap-2 text-base font-semibold text-text">
            <Ticket className="size-4 text-accent" aria-hidden="true" />
            Access tokens
          </h2>
          <p className="text-xs text-text-muted">
            Issue scoped tokens with expiration and revocation.
          </p>
        </div>
        <Button size="sm" onClick={() => setOpen(true)}>
          <Plus className="mr-1 size-4" aria-hidden="true" /> New
        </Button>
      </div>

      {error ? <p className="text-xs text-destructive">{error}</p> : null}
      {loading ? (
        <Skeleton
          name="tokens-tab-loading"
          loading
          fallback={<p className="text-xs text-text-muted">Loading…</p>}
        >
          {null}
        </Skeleton>
      ) : null}

      <Card size="sm">
        <CardHeader>
          <CardTitle className="text-sm">Tokens</CardTitle>
        </CardHeader>
        <CardContent className="space-y-2">
          {!loading && tokens.length === 0 ? (
            <p className="py-6 text-center text-xs text-text-muted">No access tokens yet.</p>
          ) : (
            tokens.map((t) => (
              <div
                key={t.uuid}
                className="flex items-center justify-between rounded-md border border-border px-3 py-2"
              >
                <div className="min-w-0">
                  <div className="truncate text-sm text-text">{t.name}</div>
                  <div className="flex flex-wrap gap-1">
                    {t.scopes.map((s) => (
                      <Badge key={s} variant="outline" className="text-[10px]">
                        {s}
                      </Badge>
                    ))}
                  </div>
                </div>
                <DropdownMenu>
                  <DropdownMenuTrigger
                    render={
                      <Button variant="ghost" size="icon" aria-label="Token actions">
                        <MoreHorizontal className="size-4" aria-hidden="true" />
                      </Button>
                    }
                  />
                  <DropdownMenuContent align="end">
                    <DropdownMenuItem
                      className="text-destructive"
                      onClick={() => {
                        void mlp
                          .revokeToken(t.uuid)
                          .then(() => load())
                          .catch((err) =>
                            setError(err instanceof Error ? err.message : String(err)),
                          );
                      }}
                    >
                      <Trash2 className="mr-2 size-4" aria-hidden="true" /> Revoke
                    </DropdownMenuItem>
                  </DropdownMenuContent>
                </DropdownMenu>
              </div>
            ))
          )}
        </CardContent>
      </Card>

      {issuedSecret ? (
        <Card size="sm">
          <CardHeader>
            <CardTitle className="text-warn text-sm">Save this token now</CardTitle>
          </CardHeader>
          <CardContent className="space-y-2">
            <p className="text-xs text-text-muted">
              The full token is shown only once. Store it safely.
            </p>
            <div className="flex gap-2">
              <code className="flex-1 truncate rounded-md border border-border bg-surface-raised px-2 py-1 font-mono text-xs text-text">
                {issuedSecret}
              </code>
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  void navigator.clipboard.writeText(issuedSecret).catch(() => undefined);
                }}
              >
                <Copy className="mr-1 size-4" aria-hidden="true" /> Copy
              </Button>
            </div>
          </CardContent>
        </Card>
      ) : null}

      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent>
          <form onSubmit={onCreate}>
            <DialogHeader>
              <DialogTitle>New access token</DialogTitle>
              <DialogDescription className="text-xs">
                Issue a token with fine-grained scopes.
              </DialogDescription>
            </DialogHeader>
            <div className="grid gap-4 py-4">
              <div className="space-y-1.5">
                <Label htmlFor="ext-token-name">Name</Label>
                <Input
                  id="ext-token-name"
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="ci-token"
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs">Scopes</Label>
                <div className="grid gap-2">
                  {SCOPES.map((scope) => (
                    <label
                      key={scope}
                      htmlFor={`ext-tok-scope-${scope}`}
                      className="flex items-center gap-2 text-xs text-text"
                    >
                      <Checkbox
                        id={`ext-tok-scope-${scope}`}
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
