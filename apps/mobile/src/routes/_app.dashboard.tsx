import { createFileRoute, useRouter } from '@tanstack/react-router';
import { FolderKanban, KeyRound, ShieldCheck } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
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
      // Aggregate secret count across all projects for the at-a-glance stat.
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

  if (projects === null) {
    return <ActivityIndicator className="mt-8" color="#42b59a" />;
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
    <View className="gap-4">
      <Text className="text-lg font-semibold text-foreground">Dashboard</Text>

      <View className="flex-row gap-3">
        {stats.map((stat) => (
          <Pressable
            key={stat.label}
            className="flex-1"
            onPress={() => router.navigate({ to: stat.to as '/' })}
          >
            <Card className="items-center gap-1 p-4">
              <stat.icon size={20} className="text-primary" />
              <Text className="text-2xl font-semibold text-foreground">{stat.value}</Text>
              <Text className="text-xs text-muted-foreground">{stat.label}</Text>
            </Card>
          </Pressable>
        ))}
      </View>

      <Card className="gap-2 p-4">
        <Text className="text-sm font-medium text-foreground">Security</Text>
        <View className="flex-row items-center justify-between">
          <Text className="text-sm text-muted-foreground">Multi-factor auth</Text>
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
