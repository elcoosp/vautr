import { createFileRoute } from '@tanstack/react-router';
import type { AuditEntry } from '@vautr/api-contract';
import { ScrollText } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ScrollView, View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Card } from '../../components/ui/card';
import { EmptyState } from '../../components/ui/empty-state';
import { ThemedIcon } from '../../components/ui/icon';
import { Text } from '../../components/ui/text';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/audit')({
  component: AuditLogScreen,
});

function formatTimestamp(ms: number): string {
  const d = new Date(ms);
  if (Number.isNaN(d.getTime())) return '—';
  return d.toLocaleString();
}

function AuditLogScreen() {
  const [entries, setEntries] = useState<AuditEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      setEntries(await services.mlp.auditList({ limit: 100, offset: 0 }));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load the security log.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  return (
    <View className="gap-4">
      <View className="flex-row items-center gap-2">
        <ThemedIcon icon={ScrollText} size={20} tone="primary" />
        <Text variant="h3">Security log</Text>
      </View>
      <Text variant="muted">
        Server-side audit timeline (logins, key rotations, account changes). Metadata only — never
        contains secret values.
      </Text>

      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      {entries === null ? (
        <Text variant="muted">Loading…</Text>
      ) : entries.length === 0 ? (
        <EmptyState icon={ScrollText} title="No audit events yet." />
      ) : (
        <ScrollView className="max-h-96 gap-2" contentContainerClassName="gap-2">
          {entries.map((e) => (
            <Card key={e.id} className="gap-1 p-3">
              <View className="flex-row flex-wrap items-baseline gap-x-3 gap-y-1">
                <Text variant="small" className="font-mono text-muted-foreground">
                  {formatTimestamp(e.created_at)}
                </Text>
                <Text variant="p" className="font-medium">
                  {e.action}
                </Text>
                {e.event_type ? (
                  <Text variant="small" className="text-muted-foreground">
                    ({e.event_type})
                  </Text>
                ) : null}
                {e.actor ? (
                  <Text variant="small" className="text-muted-foreground">
                    by {e.actor}
                  </Text>
                ) : null}
              </View>
              {e.detail ? (
                <Text variant="small" className="text-muted-foreground">
                  {e.detail}
                </Text>
              ) : null}
            </Card>
          ))}
        </ScrollView>
      )}
    </View>
  );
}
