import { createFileRoute } from '@tanstack/react-router';
import type { AuditEntry } from '@vautr/api-contract';
import { ScrollText } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { MlpApiError, mlp } from '@/lib/mlp';

export const Route = createFileRoute('/_authed/audit')({
  component: AuditLogPage,
});

function AuditLogPage() {
  const [entries, setEntries] = useState<AuditEntry[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setBusy(true);
    try {
      const res = await mlp.auditList({ limit: 100, offset: 0 });
      setEntries(res);
      setError(null);
    } catch (err) {
      setError(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const onDownload = () => {
    const blob = new Blob([JSON.stringify(entries, null, 2)], {
      type: 'application/json',
    });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `vautr-audit-${Date.now()}.json`;
    a.click();
    URL.revokeObjectURL(url);
  };

  return (
    <div className="space-y-6 p-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="flex items-center gap-2 text-2xl font-semibold text-text">
            <ScrollText className="size-5 text-accent" aria-hidden="true" />
            Security log
          </h1>
          <p className="text-sm text-text-muted">
            Server-side audit timeline (logins, key rotations, account changes). Metadata only —
            never contains secret values.
          </p>
        </div>
        <Button onClick={onDownload}>
          Download JSON
        </Button>
      </div>

      {error ? <p className="text-sm text-danger">{error}</p> : null}
      {!busy && !error && entries.length === 0 ? (
        <p className="text-sm text-text-muted">No audit events yet.</p>
      ) : null}

      <Card>
        <CardHeader>
          <CardTitle>Audit events</CardTitle>
          <CardDescription>
            {entries.length} event{entries.length === 1 ? '' : 's'} loaded.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-2">
          {entries.map((e) => (
            <div
              key={e.id}
              className="flex flex-wrap items-baseline gap-x-3 gap-y-1 rounded border border-border px-3 py-2 text-sm"
            >
              <span className="font-mono text-xs text-text-muted">
                {new Date(e.created_at).toLocaleString()}
              </span>
              <span className="font-medium text-text">{e.action}</span>
              {e.event_type ? (
                <span className="text-xs text-text-muted">({e.event_type})</span>
              ) : null}
              {e.actor ? <span className="text-xs text-text-muted">by {e.actor}</span> : null}
              {e.detail ? <span className="w-full text-xs text-text-muted">{e.detail}</span> : null}
            </div>
          ))}
        </CardContent>
      </Card>
    </div>
  );
}
