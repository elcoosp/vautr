import { createFileRoute } from '@tanstack/react-router';
import { useEffect, useRef, useState } from 'react';
import { View } from 'react-native';
import Animated, { FadeInDown } from 'react-native-reanimated';
import { Button, ButtonText } from '../../components/ui/button';
import { Card } from '../../components/ui/card';
import { Separator } from '../../components/ui/separator';
import { Text } from '../../components/ui/text';
import { useToast } from '../../components/ui/toast';
import { describeBiometricDenial, requireBiometric } from '../../lib/biometrics';
import { services } from '../../lib/client';
import { useSession } from '../../lib/session';
import { useVaultLock } from '../../lib/vaultLock';
import { clearSensitiveClipboard, copySensitiveToClipboard } from '../../src/lib/secureClipboard';
import { triggerReplayOnboarding } from '../../src/onboarding/OnboardingFlow';
import { triggerReplayTour } from '../../src/tour/TourOverlay';
import { shareKit } from '../lib/kit';

export const Route = createFileRoute('/_app/settings')({
  component: SettingsScreen,
});

function SettingsScreen() {
  const username = useSession((s) => s.username);
  const locked = useVaultLock((s) => s.locked);
  const toast = useToast();
  const [kitRevealed, setKitRevealed] = useState(false);
  const copiedTokenRef = useRef<number | null>(null);
  const kit = services.auth.getEmergencyKit();

  // If the screen unmounts while the mnemonic is still on the clipboard,
  // clear it immediately (the epoch guard skips this if a newer copy won).
  useEffect(() => {
    return () => clearSensitiveClipboard(copiedTokenRef.current ?? undefined);
  }, []);

  const revealKit = async () => {
    if (locked) {
      toast.show({
        title: 'Vault locked',
        description: 'Unlock the vault with biometrics (fingerprint button in the header) first.',
        variant: 'destructive',
      });
      return;
    }
    const gate = await requireBiometric('Reveal Emergency Kit');
    if (!gate.success) {
      toast.show({
        title: 'Reveal blocked',
        description: describeBiometricDenial(gate.reason, 'revealing the Emergency Kit'),
        variant: 'destructive',
      });
      return;
    }
    setKitRevealed(true);
  };

  const copyMnemonic = () => {
    if (!kit) return;
    // Auto-clears after ~45s; never cleared early by a newer copy (epoch guard).
    copiedTokenRef.current = copySensitiveToClipboard(kit.mnemonic);
    toast.show({
      title: 'Copied',
      description: 'Recovery Key copied — the clipboard is cleared automatically in ~45 seconds.',
    });
  };

  return (
    <Animated.View entering={FadeInDown.duration(300).springify()} className="gap-4">
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
            <Button variant="outline" disabled={locked} onPress={() => void revealKit()}>
              <ButtonText>{kitRevealed ? 'Hide' : 'Reveal'}</ButtonText>
            </Button>
            {locked ? (
              <Text variant="tiny" className="text-muted-foreground">
                Vault locked after inactivity — unlock with biometrics in the header to reveal.
              </Text>
            ) : null}
            {kitRevealed && !locked ? (
              <Text variant="tiny" className="font-mono text-foreground" selectable>
                {kit.mnemonic}
              </Text>
            ) : null}
            <View className="flex-row gap-2">
              <Button variant="outline" disabled={locked || !kitRevealed} onPress={copyMnemonic}>
                <ButtonText>Copy</ButtonText>
              </Button>
              <Button
                variant="outline"
                disabled={locked || !kitRevealed}
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
    </Animated.View>
  );
}
