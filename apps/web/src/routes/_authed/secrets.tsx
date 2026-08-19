import { createFileRoute, Link } from '@tanstack/react-router';
import type { Project, Secret } from '@vautr/api-contract';
import { Skeleton } from 'boneyard-js/react';
import { Eye, EyeOff, KeyRound, Lock } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { EmptyState } from '@/components/EmptyState';
import { Badge } from '@/components/ui/badge';
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
import { performAction, release, reveal } from '@/lib/client';
import { MlpApiError, mlp } from '@/lib/mlp';

export const Route = createFileRoute('/_authed/secrets')({
  component: SecretsManagerPage,
});

interface Row {
  project: Project;
  secret: Secret;
}

function SecretsManagerPage() {
  const [rows, setRows] = useState<Row[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [copied, setCopied] = useState<Record<string, boolean>>({});
  const [revealError, setRevealError] = useState<Record<string, string>>({});

  const load = useCallback(async () => {
    try {
      const projects = await mlp.listProjects();
      const collected: Row[] = [];
      for (const p of projects.projects) {
        try {
          const res = await mlp.listSecrets(p.uuid);
          for (const s of res.secrets) collected.push({ project: p, secret: s });
        } catch {
          // skip projects we cannot list
        }
      }
      setRows(collected);
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

  // ZK reveal: the plaintext is decrypted in the wasm `WebClient` behind an
  // opaque handle and copied to the clipboard via `perform_action`; it never
  // enters React state (data.md §1 rule 4 / VTR-062).
  const onReveal = async (uuid: string) => {
    try {
      const handle = await reveal(uuid);
      try {
        await performAction({ type: 'CopyToClipboard', handle });
        setCopied((prev) => ({ ...prev, [uuid]: true }));
        window.setTimeout(() => setCopied((prev) => ({ ...prev, [uuid]: false })), 1500);
        setRevealError((prev) => ({ ...prev, [uuid]: '' }));
      } finally {
        await release(handle);
      }
    } catch (err) {
      setRevealError((prev) => ({
        ...prev,
        [uuid]: err instanceof MlpApiError ? err.message : String(err),
      }));
    }
  };

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold text-text">Secrets</h1>
        <p className="text-sm text-text-muted">
          Secrets are project-scoped. Revealing a value requires the{' '}
          <code className="rounded bg-surface-raised px-1">secrets:reveal</code> scope.
        </p>
      </div>

      {error ? <p className="text-sm text-danger">{error}</p> : null}
      {loading ? (
        <Skeleton
          name="secrets-loading"
          loading
          fallback={<p className="text-sm text-text-muted">Loading…</p>}
        >
          {null}
        </Skeleton>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle className="flex items-center gap-2">
            <Lock className="size-4 text-accent" aria-hidden="true" />
            All secrets
          </CardTitle>
        </CardHeader>
        <CardContent>
          {!loading && rows.length === 0 ? (
            <EmptyState
              icon={KeyRound}
              title="No secrets yet."
              description={
                <span>
                  No secrets found across your projects.{' '}
                  <Link to="/projects" search={{ create: false }} className="text-accent underline">
                    Open a project
                  </Link>{' '}
                  to add one.
                </span>
              }
            />
          ) : (
            <Table>
              <TableHeader>
                <TableRow>
                  <TableHead>Project</TableHead>
                  <TableHead>Key</TableHead>
                  <TableHead>Version</TableHead>
                  <TableHead className="w-28">Value</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {rows.map(({ project, secret }) => (
                  <TableRow key={secret.uuid}>
                    <TableCell>
                      <Link
                        to="/projects/$uuid"
                        params={{ uuid: project.uuid }}
                        className="text-accent hover:underline"
                      >
                        {project.name}
                      </Link>
                      <Badge variant="outline" className="ml-2">
                        {project.type}
                      </Badge>
                    </TableCell>
                    <TableCell className="font-mono text-text">{secret.key}</TableCell>
                    <TableCell className="text-text-muted">{secret.version}</TableCell>
                    <TableCell>
                      <Button
                        size="sm"
                        variant="outline"
                        onClick={() => void onReveal(secret.uuid)}
                      >
                        {copied[secret.uuid] ? (
                          <EyeOff className="mr-1 size-4" aria-hidden="true" />
                        ) : (
                          <Eye className="mr-1 size-4" aria-hidden="true" />
                        )}
                        {copied[secret.uuid] ? 'Copied' : 'Reveal'}
                      </Button>
                    </TableCell>
                  </TableRow>
                ))}
              </TableBody>
            </Table>
          )}

          {Object.values(revealError).some(Boolean) ? (
            <div className="mt-4 space-y-2">
              {rows.map(({ secret }) =>
                revealError[secret.uuid] ? (
                  <p
                    key={secret.uuid}
                    role="alert"
                    className="rounded-md border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger"
                  >
                    Reveal denied for {secret.key}: {revealError[secret.uuid]}
                  </p>
                ) : null,
              )}
            </div>
          ) : null}
        </CardContent>
      </Card>
    </div>
  );
}
