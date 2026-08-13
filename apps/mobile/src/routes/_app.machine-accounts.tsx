import { createFileRoute } from '@tanstack/react-router';
import { Bot } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { useToast } from '../../components/ui/toast';
import type { AccessScope, MachineAccount } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/machine-accounts')({
  component: MachineAccountsScreen,
});

const SCOPES: AccessScope[] = ['secrets:read', 'secrets:write', 'secrets:reveal'];

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
        <Text className="text-lg font-semibold text-foreground">Machine accounts</Text>
        {machines ? <Badge variant="outline">{machines.length}</Badge> : null}
      </View>

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
        <Text className="text-sm font-medium text-foreground">Create machine account</Text>
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
          <Text className="text-sm text-muted-foreground">Scopes</Text>
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
        <ActivityIndicator className="mt-4" color="#42b59a" />
      ) : machines.length === 0 ? (
        <Text className="text-sm text-muted-foreground">No machine accounts yet.</Text>
      ) : (
        machines.map((m) => (
          <Card key={m.uuid} className="flex-row items-center justify-between p-4">
            <View className="flex-row items-center gap-2">
              <Bot size={18} className="text-muted-foreground" />
              <Text className="text-base font-medium text-foreground">{m.name}</Text>
            </View>
            <Badge variant={m.status === 'active' ? 'default' : 'secondary'}>{m.status}</Badge>
          </Card>
        ))
      )}
    </View>
  );
}
