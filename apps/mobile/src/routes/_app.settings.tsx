import { createFileRoute } from '@tanstack/react-router';
import { useState } from 'react';
import { Clipboard, Pressable, Text, View } from 'react-native';
import { Card } from '../../components/ui/card';
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
      <Text className="text-lg font-semibold text-foreground">Settings</Text>

      <Card className="gap-2 p-4">
        <Text className="text-sm font-medium text-foreground">App</Text>
        {username ? (
          <Text className="text-xs text-muted-foreground">Signed in as {username}</Text>
        ) : null}
        <Text className="text-sm text-muted-foreground">
          Vautr mobile unlocks with biometrics and keeps your vault encrypted on device. Use the
          desktop or web client for machine accounts, access tokens, and backup restore.
        </Text>
      </Card>

      <Card className="gap-2 p-4">
        <Text className="text-sm font-medium text-foreground">Emergency Kit</Text>
        <Text className="text-xs text-muted-foreground">
          Your Recovery Key recovers this account if you forget your master password. It is stored
          encrypted on this device and never sent to the server.
        </Text>
        {kit ? (
          <View className="gap-2">
            <Pressable
              className="self-start rounded-md border border-input bg-background px-4 py-2 active:opacity-80"
              onPress={() => setKitRevealed((v) => !v)}
            >
              <Text className="text-sm text-foreground">{kitRevealed ? 'Hide' : 'Reveal'}</Text>
            </Pressable>
            {kitRevealed ? (
              <Text className="select-all font-mono text-xs text-foreground">{kit.mnemonic}</Text>
            ) : null}
            <View className="flex-row gap-2">
              <Pressable
                className="rounded-md border border-input bg-background px-4 py-2 active:opacity-80"
                onPress={() => Clipboard.setString(kit.mnemonic)}
              >
                <Text className="text-sm text-foreground">Copy</Text>
              </Pressable>
              <Pressable
                className="rounded-md border border-input bg-background px-4 py-2 active:opacity-80"
                onPress={() => void shareKit(kit.mnemonic, username || 'you')}
              >
                <Text className="text-sm text-foreground">Share</Text>
              </Pressable>
            </View>
          </View>
        ) : (
          <Text className="text-xs text-muted-foreground">
            No Emergency Kit found for this account. If you registered before kits were enabled, you
            can generate one by rotating your recovery key.
          </Text>
        )}
      </Card>

      <Card className="gap-2 p-4">
        <Text className="text-sm font-medium text-foreground">Onboarding tour</Text>
        <Text className="text-xs text-muted-foreground">
          Re-run the first-run guided setup (create a vault, save your Emergency Kit, add a secret).
        </Text>
        <Pressable
          className="mt-1 self-start rounded-md border border-input bg-background px-4 py-2 active:opacity-80"
          onPress={() => triggerReplayOnboarding()}
        >
          <Text className="text-sm text-foreground">Replay onboarding</Text>
        </Pressable>
      </Card>

      <Card className="gap-2 p-4">
        <Text className="text-sm font-medium text-foreground">Feature tour</Text>
        <Text className="text-xs text-muted-foreground">
          A quick walkthrough of the main surfaces (vault, add a secret, Emergency Kit, audit log).
        </Text>
        <Pressable
          className="mt-1 self-start rounded-md border border-input bg-background px-4 py-2 active:opacity-80"
          onPress={() => triggerReplayTour()}
        >
          <Text className="text-sm text-foreground">Replay tour</Text>
        </Pressable>
      </Card>
    </View>
  );
}
