import type { BackupStatus } from '@vautr/api-contract';
import type { VautrMlpClient } from '@vautr/client-sdk';
import { ArrowLeftRight, Download, Upload } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';

/**
 * Import / export backup archive (VTR-064 / VTR-039). Mirrors the web
 * `import-export` route, adapted to the extension popup surface.
 */
export function ImportExportTab({ mlp }: { mlp: VautrMlpClient }) {
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [includeSecrets, setIncludeSecrets] = useState(true);
  const [archiveB64, setArchiveB64] = useState('');
  const [busy, setBusy] = useState<'export' | 'restore' | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const res = await mlp.backupStatus();
      setStatus(res);
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [mlp]);

  useEffect(() => {
    void load();
  }, [load]);

  const onExport = async () => {
    setBusy('export');
    try {
      await mlp.backupExport({ include_secrets: includeSecrets });
      setError(null);
      void load();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const onRestore = async () => {
    if (!archiveB64.trim()) {
      setError('Paste a base64 backup archive to restore');
      return;
    }
    setBusy('restore');
    try {
      await mlp.backupRestore({ archive_base64: archiveB64.trim() });
      setArchiveB64('');
      setError(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-4">
      <div>
        <h2 className="flex items-center gap-2 text-base font-semibold text-text">
          <ArrowLeftRight className="size-4 text-accent" aria-hidden="true" />
          Import / export
        </h2>
        <p className="text-xs text-text-muted">Backup and restore your organization data.</p>
      </div>

      {error ? <p className="text-xs text-destructive">{error}</p> : null}

      {status ? (
        <Card size="sm">
          <CardHeader>
            <CardTitle className="text-sm">Backup status</CardTitle>
            <CardDescription className="text-xs">Automated and on-demand backups.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap items-center gap-3 text-xs">
            <span className={status.enabled ? 'text-text' : 'text-text-muted'}>
              {status.enabled ? 'enabled' : 'disabled'}
            </span>
            {status.last_backup_at ? (
              <span className="text-text-muted">
                Last backup {new Date(status.last_backup_at).toLocaleString()}
              </span>
            ) : null}
            {status.last_restore_test_status ? (
              <span className="text-text-muted">
                Restore test: {status.last_restore_test_status}
              </span>
            ) : null}
          </CardContent>
        </Card>
      ) : null}

      <div className="grid gap-4">
        <Card size="sm">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-sm">
              <Download className="size-4 text-accent" aria-hidden="true" />
              Export backup
            </CardTitle>
            <CardDescription className="text-xs">
              Create an encrypted backup archive of the current state.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <label
              htmlFor="ext-include-secrets"
              className="flex items-center gap-2 text-xs text-text"
            >
              <Checkbox
                id="ext-include-secrets"
                checked={includeSecrets}
                onCheckedChange={(v) => setIncludeSecrets(!!v)}
              />
              Include secret values
            </label>
            <Button size="sm" onClick={() => void onExport()} disabled={busy !== null}>
              {busy === 'export' ? 'Exporting…' : 'Export backup'}
            </Button>
          </CardContent>
        </Card>

        <Card size="sm">
          <CardHeader>
            <CardTitle className="flex items-center gap-2 text-sm">
              <Upload className="size-4 text-accent" aria-hidden="true" />
              Restore backup
            </CardTitle>
            <CardDescription className="text-xs">Restore from a base64 archive.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-3">
            <div className="space-y-1.5">
              <Label htmlFor="ext-restore-archive" className="text-xs">
                Archive (base64)
              </Label>
              <Textarea
                id="ext-restore-archive"
                value={archiveB64}
                onChange={(e) => setArchiveB64(e.target.value)}
                placeholder="Paste base64 archive here…"
                rows={4}
              />
            </div>
            <Button
              size="sm"
              variant="outline"
              onClick={() => void onRestore()}
              disabled={busy !== null}
            >
              {busy === 'restore' ? 'Restoring…' : 'Restore'}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
