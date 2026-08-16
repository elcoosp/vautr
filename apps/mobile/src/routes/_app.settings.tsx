import { createFileRoute } from '@tanstack/react-router';
import { useState } from 'react';
import { Clipboard, View } from 'react-native';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Separator } from '../../components/ui/separator';
import { Text } from '../../components/ui/text';
import { services } from '../../lib/client';
import { useSession } from '../../lib/session';
import { triggerReplayOnboarding } from '../../src/onboarding/OnboardingFlow';
import { triggerReplayTour } from '../../src/tour/TourOverlay';
import { shareKit } from '../lib/kit';

export const Route = createFileRoute('/_app/settings')({
  component: SettingsScreen,
});

function SettingsScreen() {
  const username = useSession((s) => s.username);
  const [kitRevealed, setKitRevealed] = useState(false);
  const kit = services.auth.getEmergencyKit();

  return (
    <View className="gap-4">
      <Text variant="h3">Settings</Text>

      <Card className="gap-2 p-4">
        <Text variant="label">App</Text>
        {username ? <Text variant="tiny">Signed in as {username}</Text> : null}
        <Text variant="muted">
          Vautr mobile unlocks with biometrics and keeps your vault encrypted on device. Use the
          desktop or web client for machine accounts, access tokens, and backup restore.
        </Text>
      </Card>

      <Card className="gap-2 p-4">
        <Text variant="label">Emergency Kit</Text>
        <Text variant="muted">
          Your Recovery Key recovers this account if you forget your master password. It is stored
          encrypted on this device and never sent to the server.
        </Text>
        {kit ? (
          <View className="gap-2">
            <Button variant="outline" onPress={() => setKitRevealed((v) => !v)}>
              <ButtonText>{kitRevealed ? 'Hide' : 'Reveal'}</ButtonText>
            </Button>
            {kitRevealed ? (
              <Text variant="tiny" className="font-mono text-foreground" selectable>
                {kit.mnemonic}
              </Text>
            ) : null}
            <View className="flex-row gap-2">
              <Button variant="outline" onPress={() => Clipboard.setString(kit.mnemonic)}>
                <ButtonText>Copy</ButtonText>
              </Button>
              <Button
                variant="outline"
                onPress={() => void shareKit(kit.mnemonic, username || 'you')}
              >
                <ButtonText>Share</ButtonText>
              </Button>
            </View>
          </View>
        ) : (
          <Text variant="tiny">
            No Emergency Kit found for this account. If you registered before kits were enabled, you
            can generate one by rotating your recovery key.
          </Text>
        )}
      </Card>

      <Card className="gap-2 p-4">
        <Text variant="label">Onboarding tour</Text>
        <Text variant="muted">
          Re-run the first-run guided setup (create a vault, save your Emergency Kit, add a secret).
        </Text>
        <Button
          variant="outline"
          className="mt-1 self-start"
          onPress={() => triggerReplayOnboarding()}
        >
          <ButtonText>Replay onboarding</ButtonText>
        </Button>
      </Card>

      <Card className="gap-2 p-4">
        <Text variant="label">Feature tour</Text>
        <Text variant="muted">
          A quick walkthrough of the main surfaces (vault, add a secret, Emergency Kit, audit log).
        </Text>
        <Button variant="outline" className="mt-1 self-start" onPress={() => triggerReplayTour()}>
          <ButtonText>Replay tour</ButtonText>
        </Button>
      </Card>

      <Separator />
    </View>
  );
}
