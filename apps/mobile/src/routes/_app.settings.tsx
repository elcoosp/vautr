import { createFileRoute } from '@tanstack/react-router';
import { useCallback, useEffect, useState } from 'react';
import { Text, View } from 'react-native';

import { services } from '../../lib/client';
import type { AccessScope, MachineAccount } from '../../lib/api';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { useToast } from '../../components/ui/toast';

export const Route = createFileRoute('/_app/settings')({
  component: SettingsScreen,
});

const SCOPES: AccessScope[] = ['secrets:read', 'secrets:write', 'secrets:reveal'];

function SettingsScreen() {
  const toast = useToast();
  const [machines, setMachines] = useState<MachineAccount[] | null>(null);
  const [tokens, setTokens] = useState<{ uuid: string; name: string; scopes: AccessScope[] }[]>([]);
  const [name, setName] = useState('');
  const [scopes, setScopes] = useState<AccessScope[]>(['secrets:read']);
  const [createdToken, setCreatedToken] = useState<{ token: string; token_id: string } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const [m, t] = await Promise.all([
        services.api.listMachineAccounts(),
        services.api.listTokens(),
      ]);
      setMachines(m);
      setTokens(t.map((token) => ({ uuid: token.uuid, name: token.name, scopes: token.scopes })));
    } catch {
      // transient
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

  const createToken = async () => {
    setBusy(true);
    try {
      const result = await services.api.createToken({
        name: 'mobile-access',
        scopes: ['secrets:read', 'secrets:reveal'],
      });
      setCreatedToken({ token: result.token, token_id: result.token_id });
      await load();
    } catch (err) {
      toast.show({
        title: 'Failed to create token',
        description: err instanceof Error ? err.message : undefined,
        variant: 'destructive',
      });
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <Text className="text-lg font-semibold text-foreground">Settings</Text>

      <Card className="p-4 gap-3">
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

      <Card className="p-4 gap-3">
        <View className="flex-row items-center justify-between">
          <Text className="text-sm font-medium text-foreground">Access tokens</Text>
          <Button variant="outline" size="sm" onPress={() => void createToken()}>
            <ButtonText>New token</ButtonText>
          </Button>
        </View>
        {createdToken ? (
          <View className="gap-1">
            <Text className="text-xs text-muted-foreground">
              Copy your token now — it won't be shown again.
            </Text>
            <Text className="text-xs text-foreground" selectable>
              {createdToken.token}
            </Text>
          </View>
        ) : null}
        {tokens.map((token) => (
          <View key={token.uuid} className="flex-row items-center justify-between">
            <Text className="text-sm text-foreground">{token.name}</Text>
            <Text className="text-xs text-muted-foreground">{token.scopes.join(', ')}</Text>
          </View>
        ))}
      </Card>

      <Card className="p-4 gap-2">
        <Text className="text-sm font-medium text-foreground">Machine accounts</Text>
        {machines === null ? (
          <Text className="text-sm text-muted-foreground">Loading…</Text>
        ) : machines.length === 0 ? (
          <Text className="text-sm text-muted-foreground">None yet.</Text>
        ) : (
          machines.map((m) => (
            <View key={m.uuid} className="flex-row items-center justify-between">
              <Text className="text-sm text-foreground">{m.name}</Text>
              <Badge variant={m.status === 'active' ? 'default' : 'secondary'}>{m.status}</Badge>
            </View>
          ))
        )}
      </Card>
    </View>
  );
}
