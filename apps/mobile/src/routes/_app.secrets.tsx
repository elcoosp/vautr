import { createFileRoute, useRouter } from '@tanstack/react-router';
import { Eye, EyeOff } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { useToast } from '../../components/ui/toast';
import type { Project, Secret } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/secrets')({
  component: SecretsScreen,
});

interface SecretsEntry {
  project: Project;
  secret: Secret;
}

function SecretsScreen() {
  const router = useRouter();
  const toast = useToast();
  const [entries, setEntries] = useState<SecretsEntry[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [revealed, setRevealed] = useState<Record<string, string>>({});
  const [denied, setDenied] = useState<Record<string, string>>({});

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

  if (error) {
    return (
      <View className="gap-3">
        <Text accessibilityRole="alert" className="text-sm text-destructive">
          {error}
        </Text>
        <Button onPress={() => void load()}>
          <ButtonText>Retry</ButtonText>
        </Button>
      </View>
    );
  }

  if (entries === null) {
    return <ActivityIndicator className="mt-8" color="#42b59a" />;
  }

  const reveal = async (entry: SecretsEntry) => {
    if (revealed[entry.secret.uuid] || denied[entry.secret.uuid]) {
      setRevealed((current) => {
        const next = { ...current };
        delete next[entry.secret.uuid];
        return next;
      });
      setDenied((current) => {
        const next = { ...current };
        delete next[entry.secret.uuid];
        return next;
      });
      return;
    }
    try {
      // Mobile is HTTP-only: the server returns the *ciphertext*, not plaintext.
      // Client-side decryption needs the vautr-wasm DEK, which the mobile app
      // does not ship, so reveal shows a locked state rather than fake-decrypt.
      await services.api.revealSecret(entry.secret.uuid);
      setDenied((current) => ({
        ...current,
        [entry.secret.uuid]:
          'Encrypted on this device — open Vautr desktop or web to view the secret.',
      }));
    } catch (err) {
      setDenied((current) => ({
        ...current,
        [entry.secret.uuid]: err instanceof Error ? err.message : 'Reveal denied.',
      }));
      toast.show({
        title: 'Reveal denied',
        description: err instanceof Error ? err.message : 'You lack the secrets:reveal permission.',
        variant: 'destructive',
      });
    }
  };

  return (
    <View className="gap-4">
      <View className="flex-row items-center justify-between">
        <Text className="text-lg font-semibold text-foreground">Secrets</Text>
        <Badge variant="outline">{entries.length}</Badge>
      </View>

      {entries.length === 0 ? (
        <Text className="text-sm text-muted-foreground">
          No secrets yet. Add secrets inside a project.
        </Text>
      ) : (
        entries.map((entry) => (
          <Card key={entry.secret.uuid} className="p-4">
            <Pressable onPress={() => void reveal(entry)}>
              <View className="flex-row items-center justify-between">
                <View className="flex-1 pr-2">
                  <Text className="text-base font-medium text-foreground">{entry.secret.key}</Text>
                  <Text className="mt-0.5 text-xs text-muted-foreground">
                    {entry.project.name} · v{entry.secret.version}
                  </Text>
                  {denied[entry.secret.uuid] ? (
                    <Text className="mt-1 rounded border border-destructive/40 bg-destructive/10 px-2 py-1 text-xs text-destructive">
                      {denied[entry.secret.uuid]}
                    </Text>
                  ) : null}
                </View>
                {denied[entry.secret.uuid] ? (
                  <EyeOff size={18} className="text-muted-foreground" />
                ) : (
                  <Eye size={18} className="text-muted-foreground" />
                )}
              </View>
            </Pressable>
          </Card>
        ))
      )}

      <Button variant="outline" onPress={() => router.navigate({ to: '/' })}>
        <ButtonText>Manage in Projects</ButtonText>
      </Button>
    </View>
  );
}
