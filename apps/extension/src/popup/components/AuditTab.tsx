import type { AuditEntry } from '@vautr/api-contract';
import { ScrollText } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { getPopupMlpClient } from '../popupClient';
import { EmptyState } from './EmptyState';

export function AuditTab() {
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const mlp = await getPopupMlpClient();
      setEntries(await mlp.auditList({ limit: 100, offset: 0 }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load the security log.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <div className="space-y-3">
      <div>
        <h2 className="flex items-center gap-2 text-base font-semibold text-text">
          <ScrollText className="size-4 text-accent" aria-hidden="true" />
          Security log
        </h2>
        <p className="text-xs text-text-muted">
          Server-side audit timeline (logins, key rotations, account changes). Metadata only — never
          contains secret values.
        </p>
      </div>

      {error ? <p className="text-sm text-destructive">{error}</p> : null}

      {entries === null ? (
        <p className="text-sm text-text-muted">Loading…</p>
      ) : entries.length === 0 ? (
        <EmptyState icon={ScrollText} title="No audit events yet." />
      ) : (
        <div className="space-y-2">
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
        </div>
      )}
    </div>
  );
}
