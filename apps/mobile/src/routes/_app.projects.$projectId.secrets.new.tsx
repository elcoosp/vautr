import { createFileRoute } from '@tanstack/react-router';
import { useState } from 'react';
import { View } from 'react-native';
import { Alert, AlertDescription } from '../../components/ui/alert';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Input } from '../../components/ui/input';
import { Label } from '../../components/ui/label';
import { Text } from '../../components/ui/text';

export const Route = createFileRoute('/_app/projects/$projectId/secrets/new')({
  component: NewSecretScreen,
});

/**
 * VTRFIX-SEC-C01: mobile secret creation is gated OFF. The linked native module
 * does not yet expose item encryption, so there is no way to produce a
 * `value_ciphertext` on device — uploading the raw value would break the
 * zero-knowledge model. The form stays visible (so the UX gap is discoverable)
 * but the submit action is disabled and nothing is ever sent to the server.
 * Re-enable once the uniffi core exposes item encryption (follow-up VTR).
 */
const SECRET_CREATION_DISABLED = true;

function NewSecretScreen() {
  const { projectId } = Route.useParams();
  const [key, setKey] = useState('');
  const [value, setValue] = useState('');

  return (
    <View className="gap-4">
      <Text variant="h3">New secret</Text>

      <Alert>
        <AlertDescription>
          Creating secrets on mobile is disabled until on-device encryption ships in the native
          vault core — saving here would upload the value unencrypted. Use the web or desktop client
          to add secrets to project {projectId} for now.
        </AlertDescription>
      </Alert>

      <Card className="gap-4 p-4">
        <View className="gap-1.5">
          <Label htmlFor="secret-key">Key</Label>
          <Input
            id="secret-key"
            value={key}
            onChangeText={setKey}
            placeholder="DATABASE_URL"
            autoCapitalize="characters"
            editable={!SECRET_CREATION_DISABLED}
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
            textContentType="newPassword"
            editable={!SECRET_CREATION_DISABLED}
          />
        </View>

        <Button className="self-end" disabled>
          <ButtonText>Save secret (disabled)</ButtonText>
        </Button>
      </Card>
    </View>
  );
}
