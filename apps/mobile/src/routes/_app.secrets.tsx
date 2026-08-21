import { createFileRoute, useRouter } from '@tanstack/react-router';
import { getMobileClient } from '@vautr/client-sdk/mobile';
import { KeyRound } from 'lucide-react-native';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { TextInput, View } from 'react-native';
import { SecretOverlay } from '../../components/SecretOverlay';
import { Alert, AlertDescription, AlertTitle } from '../../components/ui/alert';
import { Avatar, AvatarFallbackText } from '../../components/ui/avatar';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { EmptyState } from '../../components/ui/empty-state';
import { Separator } from '../../components/ui/separator';
import { Skeleton } from '../../components/ui/skeleton';
import { Text } from '../../components/ui/text';
import type { Project, Secret } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/secrets')({
  component: SecretsScreen,
});

interface SecretsEntry {
  project: Project;
  secret: Secret;
}

function SecretRow({ entry, hasNativeVault }: { entry: SecretsEntry; hasNativeVault: boolean }) {
  const initial = (entry.secret.key[0] ?? '?').toUpperCase();
  return (
    <Card className="flex-row items-center gap-4 p-4">
      <Avatar size={40}>
        <AvatarFallbackText>{initial}</AvatarFallbackText>
      </Avatar>
      <View className="flex-1 gap-0.5">
        <Text variant="h4">{entry.secret.key}</Text>
        <Text variant="tiny">
          {entry.project.name} · v{entry.secret.version}
        </Text>
        {hasNativeVault ? null : (
          <Text variant="tiny" className="mt-0.5 text-destructive">
            Encrypted on this device — open Vautr desktop or web to view.
          </Text>
        )}
      </View>
      <View className="items-end gap-2">
        <Badge variant="outline">{entry.project.type}</Badge>
        <SecretOverlay uuid={entry.secret.uuid} label={entry.secret.key} />
      </View>
    </Card>
  );
}

function SecretsScreen() {
  const router = useRouter();
  const [entries, setEntries] = useState<SecretsEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState('');

  const load = useCallback(async () => {
    setError(null);
    try {
      const projects = await services.api.listProjects();
      const all: SecretsEntry[] = [];
      for (const project of projects) {
        const secrets = await services.api.listProjectSecrets(project.uuid);
        for (const secret of secrets) {
          all.push({ project, secret });
        }
      }
      setEntries(all);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load secrets.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const hasNativeVault = getMobileClient() !== null;

  const q = query.trim().toLowerCase();
  const visibleEntries = useMemo(
    () =>
      q === '' || !entries
        ? entries ?? []
        : entries.filter(
            ({ project, secret }) =>
              secret.key.toLowerCase().includes(q) ||
              project.name.toLowerCase().includes(q),
          ),
    [q, entries],
  );

  return (
    <View className="gap-5">
      <View className="flex-row items-center justify-between">
        <View className="gap-0.5">
          <Text variant="h2">Secrets</Text>
          <Text variant="muted">
            {entries
              ? `${entries.length} ${entries.length === 1 ? 'entry' : 'entries'}`
              : 'Loading…'}
          </Text>
        </View>
        {entries && entries.length > 0 ? <Badge variant="outline">{entries.length}</Badge> : null}
      </View>

      <Separator />

      <TextInput
        className="rounded border border-border bg-background px-3 py-2 text-foreground"
        placeholder="Search secrets…"
        placeholderTextColor="#6b7280"
        value={query}
        onChangeText={setQuery}
        autoCapitalize="none"
        autoCorrect={false}
      />

      {error ? (
        <Alert variant="destructive">
          <AlertTitle>Couldn’t load secrets</AlertTitle>
          <AlertDescription>{error}</AlertDescription>
          <Button variant="outline" className="mt-3 self-start" onPress={() => void load()}>
            <ButtonText>Retry</ButtonText>
          </Button>
        </Alert>
      ) : entries === null ? (
        <View className="gap-3">
          {[0, 1, 2].map((i) => (
            <Skeleton key={i} className="h-[72px] w-full" />
          ))}
        </View>
      ) : entries.length === 0 ? (
        <EmptyState
          icon={KeyRound}
          title="No secrets yet."
          description="Add secrets inside a project to see them here."
          action={{ label: 'Manage in Projects', onPress: () => router.navigate({ to: '/' }) }}
        />
      ) : visibleEntries.length === 0 ? (
        <EmptyState
          icon={KeyRound}
          title="No matches."
          description={`Nothing matches “${query}”.`}
        />
      ) : (
        <View className="gap-3">
          {visibleEntries.map((entry) => (
            <SecretRow key={entry.secret.uuid} entry={entry} hasNativeVault={hasNativeVault} />
          ))}
        </View>
      )}
    </View>
  );
}
