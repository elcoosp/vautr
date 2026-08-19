import { createFileRoute } from '@tanstack/react-router';
import { Skeleton as BoneSkeleton } from 'boneyard-js/native';
import { Ticket } from 'lucide-react-native';
import { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Badge } from '../../components/ui/badge';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { EmptyState } from '../../components/ui/empty-state';
import { ThemedIcon } from '../../components/ui/icon';
import { Text } from '../../components/ui/text';
import { useToast } from '../../components/ui/toast';
import type { AccessScope, AccessToken } from '../../lib/api';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/tokens')({
  component: TokensScreen,
});

const ACCENT = '#42b59a';

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
        <Text variant="h3">Access tokens</Text>
        {tokens ? <Badge variant="outline">{tokens.length}</Badge> : null}
      </View>

      {error ? (
        <Alert variant="destructive">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : null}

      <Card className="gap-3 p-4">
        <Text variant="label">Create a new token</Text>
        <Text variant="muted">Grants read + reveal access for the mobile app.</Text>
        <Button disabled={busy} onPress={() => void createToken()}>
          <ButtonText>{busy ? 'Creating…' : 'New token'}</ButtonText>
        </Button>
        {createdToken ? (
          <View className="gap-1">
            <Text variant="small">Copy your token now — it won't be shown again.</Text>
            <Text variant="p" className="text-foreground" selectable>
              {createdToken.token}
            </Text>
          </View>
        ) : null}
      </Card>

      {tokens === null ? (
        <BoneSkeleton
          name="tokens-loading"
          loading
          fallback={<ActivityIndicator className="mt-4" color={ACCENT} />}
        >
          {null}
        </BoneSkeleton>
      ) : tokens.length === 0 ? (
        <EmptyState icon={Ticket} title="No access tokens yet." />
      ) : (
        tokens.map((token) => (
          <Card key={token.uuid} className="gap-2 p-4">
            <View className="flex-row items-center justify-between">
              <View className="flex-row items-center gap-2">
                <ThemedIcon icon={Ticket} size={18} tone="muted" />
                <Text variant="p" className="font-medium">
                  {token.name}
                </Text>
              </View>
              <Button variant="ghost" size="sm" onPress={() => void revoke(token.uuid)}>
                <ButtonText className="text-destructive">Revoke</ButtonText>
              </Button>
            </View>
            <Text variant="small">{(token.scopes as AccessScope[]).join(', ')}</Text>
          </Card>
        ))
      )}
    </View>
  );
}
