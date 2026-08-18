import { createFileRoute } from '@tanstack/react-router';
import { Skeleton as BoneSkeleton } from 'boneyard-js/native';
import { Bot } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Avatar, AvatarFallbackText } from '../../components/ui/avatar';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { ThemedIcon } from '../../components/ui/icon';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Text } from '../../components/ui/text';
import { useToast } from '../../components/ui/toast';
import type { AccessScope, MachineAccount } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/machine-accounts')({
  component: MachineAccountsScreen,
});

const SCOPES: AccessScope[] = ['secrets:read', 'secrets:write', 'secrets:reveal'];
const ACCENT = '#42b59a';

function MachineAccountsScreen() {
  const toast = useToast();
  const [machines, setMachines] = useState<MachineAccount[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [name, setName] = useState('');
  const [scopes, setScopes] = useState<AccessScope[]>(['secrets:read']);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      setMachines(await services.api.listMachineAccounts());
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load machine accounts.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const toggleScope = (scope: AccessScope) => {
    setScopes((current) =>
      current.includes(scope) ? current.filter((s) => s !== scope) : [...current, scope],
    );
  };

  const createMachine = async () => {
    if (!name.trim()) {
      toast.show({ title: 'Enter a machine account name', variant: 'destructive' });
      return;
    }
    setBusy(true);
    try {
      await services.api.createMachineAccount({ name: name.trim(), scopes });
      toast.show({ title: 'Machine account created' });
      setName('');
      await load();
    } catch (err) {
      toast.show({
        title: 'Failed to create machine account',
        description: err instanceof Error ? err.message : undefined,
        variant: 'destructive',
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <View className="flex-row items-center justify-between">
        <Text variant="h3">Machine accounts</Text>
        {machines ? <Badge variant="outline">{machines.length}</Badge> : null}
      </View>

      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Card className="gap-3 p-4">
        <Text variant="label">Create machine account</Text>
        <View className="gap-1.5">
          <Label htmlFor="ma-name">Name</Label>
          <Input
            id="ma-name"
            value={name}
            onChangeText={setName}
            placeholder="ci-deploy"
            autoCapitalize="none"
          />
        </View>
        <View className="gap-1.5">
          <Text variant="small">Scopes</Text>
          <View className="flex-row flex-wrap gap-2">
            {SCOPES.map((scope) => (
              <Badge
                key={scope}
                variant={scopes.includes(scope) ? 'default' : 'outline'}
                onPress={() => toggleScope(scope)}
              >
                {scope}
              </Badge>
            ))}
          </View>
        </View>
        <Button disabled={busy} onPress={() => void createMachine()}>
          <ButtonText>{busy ? 'Creating…' : 'Create machine account'}</ButtonText>
        </Button>
      </Card>

      {machines === null ? (
        <BoneSkeleton
          name="machine-accounts-loading"
          loading
          fallback={<ActivityIndicator className="mt-4" color={ACCENT} />}
        >
          {null}
        </BoneSkeleton>
      ) : machines.length === 0 ? (
        <Text variant="muted">No machine accounts yet.</Text>
      ) : (
        machines.map((m) => (
          <Card key={m.uuid} className="flex-row items-center justify-between p-4">
            <View className="flex-row items-center gap-2">
              <Avatar>
                <AvatarFallbackText>
                  <ThemedIcon icon={Bot} size={18} tone="muted" />
                </AvatarFallbackText>
              </Avatar>
              <Text variant="p" className="font-medium">
                {m.name}
              </Text>
            </View>
            <Badge variant={m.status === 'active' ? 'default' : 'secondary'}>{m.status}</Badge>
          </Card>
        ))
      )}
    </View>
  );
}
