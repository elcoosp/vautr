import { createFileRoute, useRouter } from '@tanstack/react-router';
import { FolderKanban, KeyRound, ShieldCheck } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { Pressable, View } from 'react-native';
import { Alert, AlertDescription, AlertTitle } from '../../components/ui/alert';
import { Avatar } from '../../components/ui/avatar';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { ThemedIcon } from '../../components/ui/icon';
import { Separator } from '../../components/ui/separator';
import { Skeleton } from '../../components/ui/skeleton';
import { Text } from '../../components/ui/text';
import type { MfaStatus, Project } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/dashboard')({
  component: DashboardScreen,
});

interface Stat {
  label: string;
  value: string;
  icon: typeof FolderKanban;
  to: string;
}

function DashboardScreen() {
  const router = useRouter();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [secretCount, setSecretCount] = useState(0);
  const [mfa, setMfa] = useState<MfaStatus | null>(null);
  const [error, setError] = useState<string | null>(null);

  const load = useCallback(async () => {
    setError(null);
    try {
      const [p, m] = await Promise.all([
        services.api.listProjects(),
        services.api.mfaStatus().catch(() => null),
      ]);
      setProjects(p);
      setMfa(m);
      let total = 0;
      for (const project of p) {
        const secrets = await services.api.listProjectSecrets(project.uuid);
        total += secrets.length;
      }
      setSecretCount(total);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load dashboard.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  if (error) {
    return (
      <Alert variant="destructive">
        <AlertTitle>Couldn’t load dashboard</AlertTitle>
        <AlertDescription>{error}</AlertDescription>
        <Button variant="outline" className="mt-3 self-start" onPress={() => void load()}>
          <ButtonText>Retry</ButtonText>
        </Button>
      </Alert>
    );
  }

  if (projects === null) {
    return (
      <View className="gap-4">
        <Skeleton className="h-24 w-full" />
        <Skeleton className="h-20 w-full" />
      </View>
    );
  }

  const stats: Stat[] = [
    { label: 'Projects', value: String(projects.length), icon: FolderKanban, to: '/' },
    { label: 'Secrets', value: String(secretCount), icon: KeyRound, to: '/secrets' },
    {
      label: 'MFA',
      value: mfa?.configured_methods.length ? 'On' : 'Off',
      icon: ShieldCheck,
      to: '/mfa',
    },
  ];

  return (
    <View className="gap-5">
      <View className="gap-0.5">
        <Text variant="h2">Dashboard</Text>
        <Text variant="muted">Your vault at a glance.</Text>
      </View>

      <View className="flex-row gap-3">
        {stats.map((stat) => (
          <Pressable
            key={stat.label}
            className="flex-1 active:opacity-90"
            onPress={() => router.navigate({ to: stat.to as '/' })}
          >
            <Card className="items-center gap-2 p-4">
              <Avatar size={36} className="bg-primary/15">
                <ThemedIcon icon={stat.icon} size={18} tone="primary" />
              </Avatar>
              <Text variant="h3">{stat.value}</Text>
              <Text variant="tiny">{stat.label}</Text>
            </Card>
          </Pressable>
        ))}
      </View>

      <Card className="gap-3 p-4">
        <Text variant="label">Security</Text>
        <Separator />
        <View className="flex-row items-center justify-between">
          <Text variant="p">Multi-factor authentication</Text>
          <Badge variant={mfa?.configured_methods.length ? 'default' : 'secondary'}>
            {mfa?.configured_methods.length ? 'Enabled' : 'Disabled'}
          </Badge>
        </View>
      </Card>

      <Button variant="outline" onPress={() => router.navigate({ to: '/generator' })}>
        <ButtonText>Generate a password</ButtonText>
      </Button>
    </View>
  );
}
