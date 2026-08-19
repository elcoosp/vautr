import { createFileRoute, useRouter } from '@tanstack/react-router';
import { useState } from 'react';
import { View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Text } from '../../components/ui/text';
import { useToast } from '../../components/ui/toast';
import { services } from '../../lib/client';

export const Route = createFileRoute('/_app/projects/$projectId/secrets/new')({
  component: NewSecretScreen,
});

function NewSecretScreen() {
  const { projectId } = Route.useParams();
  const router = useRouter();
  const toast = useToast();
  const [key, setKey] = useState('');
  const [value, setValue] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const create = async () => {
    if (!key.trim()) {
      setError('Secret key is required.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      // In a zero-knowledge client the value would be encrypted locally with the
      // DEK before upload; here we persist the value as provided (the server
      // treats it as opaque ciphertext).
      await services.api.createSecret({
        project_uuid: projectId,
        key: key.trim(),
        value_ciphertext: value,
      });
      toast.show({ title: 'Secret created', description: key.trim() });
      router.navigate({ to: '/projects/$projectId', params: { projectId } });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to create secret.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-4">
      <Text variant="h3">New secret</Text>
      <Card className="p-4 gap-4">
        <View className="gap-1.5">
          <Label htmlFor="secret-key">Key</Label>
          <Input
            id="secret-key"
            value={key}
            onChangeText={setKey}
            placeholder="DATABASE_URL"
            autoCapitalize="characters"
          />
        </View>
        <View className="gap-1.5">
          <Label htmlFor="secret-value">Value</Label>
          <Input
            id="secret-value"
            value={value}
            onChangeText={setValue}
            placeholder="Secret value"
            secureTextEntry
            textContentType="oneTimeCode"
          />
        </View>

        {error ? (
          <Alert variant="destructive">
            <AlertDescription>{error}</AlertDescription>
          </Alert>
        ) : null}

        <Button disabled={busy} onPress={() => void create()}>
          <ButtonText>{busy ? 'Saving…' : 'Save secret'}</ButtonText>
        </Button>
      </Card>
    </View>
  );
}
