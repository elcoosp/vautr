import { createFileRoute } from '@tanstack/react-router';
import { Ticket } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Text, View } from 'react-native';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { useToast } from '../../components/ui/toast';
import type { AccessScope, AccessToken } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/tokens')({
  component: TokensScreen,
});

function TokensScreen() {
  const toast = useToast();
  const [tokens, setTokens] = useState<AccessToken[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [createdToken, setCreatedToken] = useState<{ token: string; token_id: string } | null>(
    null,
  );
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setError(null);
    try {
      setTokens(await services.api.listTokens());
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to load tokens.');
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

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

  const revoke = async (uuid: string) => {
    try {
      await services.api.revokeToken(uuid);
      toast.show({ title: 'Token revoked' });
      await load();
    } catch (err) {
      toast.show({
        title: 'Failed to revoke token',
        description: err instanceof Error ? err.message : undefined,
        variant: 'destructive',
      });
    }
  };

  return (
    <View className="gap-4">
      <View className="flex-row items-center justify-between">
        <Text className="text-lg font-semibold text-foreground">Access tokens</Text>
        {tokens ? <Badge variant="outline">{tokens.length}</Badge> : null}
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
        <Text className="text-sm font-medium text-foreground">Create a new token</Text>
        <Text className="text-xs text-muted-foreground">
          Grants read + reveal access for the mobile app.
        </Text>
        <Button disabled={busy} onPress={() => void createToken()}>
          <ButtonText>{busy ? 'Creating…' : 'New token'}</ButtonText>
        </Button>
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
      </Card>

      {tokens === null ? (
        <ActivityIndicator className="mt-4" color="#42b59a" />
      ) : tokens.length === 0 ? (
        <Text className="text-sm text-muted-foreground">No access tokens yet.</Text>
      ) : (
        tokens.map((token) => (
          <Card key={token.uuid} className="gap-2 p-4">
            <View className="flex-row items-center justify-between">
              <View className="flex-row items-center gap-2">
                <Ticket size={18} className="text-muted-foreground" />
                <Text className="text-base font-medium text-foreground">{token.name}</Text>
              </View>
              <Button variant="ghost" size="sm" onPress={() => void revoke(token.uuid)}>
                <ButtonText className="text-destructive">Revoke</ButtonText>
              </Button>
            </View>
            <Text className="text-xs text-muted-foreground">
              {(token.scopes as AccessScope[]).join(', ')}
            </Text>
          </Card>
        ))
      )}
    </View>
  );
}
