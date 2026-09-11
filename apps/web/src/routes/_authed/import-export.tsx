import { createFileRoute } from '@tanstack/react-router';
import type { BackupStatus } from '@vautr/api-contract';
import { ArrowLeftRight, Download, Upload } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Textarea } from '@/components/ui/textarea';
import { MlpApiError, mlp } from '@/lib/mlp';

export const Route = createFileRoute('/_authed/import-export')({
  component: ImportExportPage,
});

function ImportExportPage() {
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
      setError(err instanceof MlpApiError ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const onExport = async () => {
    setBusy('export');
    try {
      const res = await mlp.backupExport({ include_secrets: includeSecrets });
      toast.success(
        `Backup created (${res.size_bytes ?? 'unknown'} bytes). Use the restore box with backup ID to restore.`,
      );
      void load();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  const onRestore = async () => {
    if (!archiveB64.trim()) {
      toast.error('Paste a base64 backup archive to restore');
      return;
    }
    setBusy('restore');
    try {
      const res = await mlp.backupRestore({ archive_base64: archiveB64.trim() });
      toast.success(`Restore ${res.status}: ${res.restored_records ?? 0} records`);
      setArchiveB64('');
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="flex items-center gap-2 text-2xl font-semibold text-text">
          <ArrowLeftRight className="size-5 text-accent" aria-hidden="true" />
          Import / export
        </h1>
        <p className="text-sm text-text-muted">
          Backup and restore your organization data against the live server.
        </p>
      </div>

      {error ? <p className="text-sm text-danger">{error}</p> : null}

      {status ? (
        <Card>
          <CardHeader>
            <CardTitle>Backup status</CardTitle>
            <CardDescription>Automated and on-demand backups.</CardDescription>
          </CardHeader>
          <CardContent className="flex flex-wrap items-center gap-4">
            <Badge variant={status.enabled ? 'default' : 'secondary'}>
              {status.enabled ? 'enabled' : 'disabled'}
            </Badge>
            {status.last_backup_at ? (
              <span className="text-sm text-text-muted">
                Last backup {new Date(status.last_backup_at).toLocaleString()}
              </span>
            ) : null}
            {status.last_restore_test_status ? (
              <span className="text-sm text-text-muted">
                Last restore test: {status.last_restore_test_status}
              </span>
            ) : null}
          </CardContent>
        </Card>
      ) : null}

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Download className="size-4 text-accent" aria-hidden="true" />
              Export backup
            </CardTitle>
            <CardDescription>
              Create an encrypted backup archive of the current state.
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <label htmlFor="include-secrets" className="flex items-center gap-2 text-sm text-text">
              <Checkbox
                id="include-secrets"
                checked={includeSecrets}
                onCheckedChange={(v) => setIncludeSecrets(!!v)}
              />
              Include secret values
            </label>
            <Button data-testid="export-backup-button" onClick={() => void onExport()} disabled={busy !== null}>
              {busy === 'export' ? 'Exporting…' : 'Export backup'}
            </Button>
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Upload className="size-4 text-accent" aria-hidden="true" />
              Restore backup
            </CardTitle>
            <CardDescription>Restore from a base64 archive or a backup ID.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="restore-archive">Archive (base64)</Label>
              <Textarea
                id="restore-archive"
                value={archiveB64}
                onChange={(e) => setArchiveB64(e.target.value)}
                placeholder="Paste base64 archive here…"
                rows={4}
              />
            </div>
            <Button variant="outline" onClick={() => void onRestore()} disabled={busy !== null}>
              {busy === 'restore' ? 'Restoring…' : 'Restore'}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
