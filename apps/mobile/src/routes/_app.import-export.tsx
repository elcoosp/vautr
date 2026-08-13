import { createFileRoute } from '@tanstack/react-router';
import { ArrowDownToLine, ArrowUpFromLine, ShieldCheck } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { useToast } from '../../components/ui/toast';
import type { BackupStatus } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/import-export')({
  component: BackupScreen,
});

function formatBytes(bytes?: number | null): string {
  if (!bytes) return '—';
  const mb = bytes / (1024 * 1024);
  return mb >= 1 ? `${mb.toFixed(1)} MB` : `${bytes} B`;
}

function BackupScreen() {
  const toast = useToast();
  const [status, setStatus] = useState<BackupStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [lastExport, setLastExport] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      setStatus(await services.api.getBackupStatus());
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load backup status.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const exportBackup = async () => {
    setBusy(true);
    try {
      const result = await services.api.exportBackup({ include_secrets: true });
      setLastExport(result.backup_id);
      toast.show({
        title: 'Backup created',
        description: `${formatBytes(result.size_bytes)} · checksum ${(result.checksum ?? '').slice(0, 8)}…`,
      });
      await load();
    } catch (err) {
      toast.show({
        title: 'Export failed',
        description: err instanceof Error ? err.message : 'Could not create backup.',
        variant: 'destructive',
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <Text className="text-lg font-semibold text-foreground">Import / export</Text>

      {error ? (
        <View className="gap-3">
          <Text accessibilityRole="alert" className="text-sm text-destructive">
            {error}
          </Text>
          <Button onPress={() => void load()}>
            <ButtonText>Retry</ButtonText>
          </Button>
        </View>
      ) : null}

      <Card className="gap-3 p-4">
        <View className="flex-row items-center justify-between">
          <Text className="text-sm font-medium text-foreground">Backup status</Text>
          {status ? (
            <Badge variant={status.enabled ? 'default' : 'secondary'}>
              {status.enabled ? 'Enabled' : 'Disabled'}
            </Badge>
          ) : null}
        </View>
        {status ? (
          <View className="gap-1">
            <Text className="text-xs text-muted-foreground">
              Last backup:{' '}
              {status.last_backup_at ? new Date(status.last_backup_at).toLocaleString() : 'never'}
            </Text>
            <Text className="text-xs text-muted-foreground">
              Last size: {formatBytes(status.last_backup_size_bytes)}
            </Text>
            {status.last_restore_test_status ? (
              <Text className="text-xs text-muted-foreground">
                Restore test: {status.last_restore_test_status}
              </Text>
            ) : null}
          </View>
        ) : (
          <ActivityIndicator className="mt-2" color="#42b59a" />
        )}
      </Card>

      <Card className="gap-3 p-4">
        <View className="flex-row items-center gap-2">
          <ArrowDownToLine size={18} className="text-primary" />
          <Text className="text-sm font-medium text-foreground">Export encrypted backup</Text>
        </View>
        <Text className="text-xs text-muted-foreground">
          Creates a sealed, encrypted archive of your vault on the server. The archive is keyed to
          your backup key — only you can restore it.
        </Text>
        <Button disabled={busy} onPress={() => void exportBackup()}>
          <ButtonText>{busy ? 'Exporting…' : 'Create backup'}</ButtonText>
        </Button>
        {lastExport ? (
          <Text className="text-xs text-muted-foreground">Last export id: {lastExport}</Text>
        ) : null}
      </Card>

      <Card className="gap-3 p-4">
        <View className="flex-row items-center gap-2">
          <ArrowUpFromLine size={18} className="text-muted-foreground" />
          <Text className="text-sm font-medium text-foreground">Import / restore</Text>
        </View>
        <View className="flex-row items-start gap-2">
          <ShieldCheck size={16} className="mt-0.5 text-muted-foreground" />
          <Text className="flex-1 text-xs text-muted-foreground">
            Restore is performed server-side from an existing archive. Use the desktop or web client
            to upload and restore a backup file.
          </Text>
        </View>
      </Card>
    </View>
  );
}
