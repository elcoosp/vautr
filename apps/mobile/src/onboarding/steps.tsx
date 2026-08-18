import { type OnboardingStep, type StepComponent, useOnboarding } from '@onboardjs/react';
import { useState } from 'react';
import { Clipboard, Text, TextInput, View } from 'react-native';
import { Button } from '../../components/ui/button';
import { services } from '../../lib/client';
import { shareKit } from '../lib/kit';
import { router } from '../router';

/**
 * First-run guided onboarding steps (mobile). Each step renders its own content
 * with nativewind styling; OnboardingFlow supplies the Back / Next / Skip chrome.
 * Steps drive real client surfaces — no stubs. Copy matches docs/onboarding/spec.md.
 */

function WelcomeStep() {
  return (
    <View className="gap-2">
      <Text className="text-xl font-semibold text-foreground">Welcome to Vautr</Text>
      <Text className="text-sm text-muted-foreground">
        Vautr is a zero-knowledge vault: your secrets are encrypted on your device and the server
        never sees them. Let&apos;s set up the essentials in about a minute.
      </Text>
    </View>
  );
}

function CreateVaultStep() {
  const { next, updateContext, state } = useOnboarding();
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async () => {
    setBusy(true);
    setError(null);
    try {
      const project = await services.api.createProject({ name: name.trim() || 'My Vault' });
      updateContext({ flowData: { vaultName: project.name } });
      void router.navigate({ to: '/projects/$projectId', params: { projectId: project.uuid } });
      next();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not create vault.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <View className="gap-3">
      <Text className="text-xl font-semibold text-foreground">Create your first vault</Text>
      <Text className="text-sm text-muted-foreground">
        A vault (project) groups your secrets. You can create more later.
      </Text>
      <TextInput
        className="rounded-md border border-input bg-background px-3 py-2 text-foreground"
        placeholder="My Vault"
        placeholderTextColor="#9ca3af"
        value={state?.context.flowData.vaultName ?? name}
        onChangeText={setName}
      />
      {error ? <Text className="text-sm text-destructive">{error}</Text> : null}
      <Button onPress={onSubmit} disabled={busy}>
        <Text className="text-primary-foreground">
          {busy ? 'Creating…' : 'Create vault & continue'}
        </Text>
      </Button>
    </View>
  );
}

function EmergencyKitStep() {
  const { next } = useOnboarding();
  const [revealed, setRevealed] = useState(false);
  const mnemonic = services.auth.pendingRecoveryMnemonic;
  const words = mnemonic ? mnemonic.split(/\s+/).filter(Boolean) : [];

  const download = async () => {
    if (!mnemonic) return;
    const email = (await services.auth.getUsername()) || 'you';
    await shareKit(mnemonic, email);
    services.auth.pendingRecoveryMnemonic = null;
  };

  return (
    <View className="gap-3">
      <Text className="text-xl font-semibold text-foreground">Save your Emergency Kit</Text>
      <Text className="text-sm text-muted-foreground">
        Your Recovery Key is the only way to recover your account if you forget your master
        password. It was generated and encrypted on this device. Store it somewhere safe.
      </Text>
      {mnemonic ? (
        <View className="gap-2 rounded-lg border border-border bg-background p-3">
          <View className="flex-row items-center justify-between">
            <Text className="text-xs font-medium text-muted-foreground">24-word Recovery Key</Text>
            <Button variant="ghost" onPress={() => setRevealed((v) => !v)}>
              <Text className="text-foreground">{revealed ? 'Hide' : 'Reveal'}</Text>
            </Button>
          </View>
          {revealed ? (
            <Text className="select-all font-mono text-sm text-foreground">{mnemonic}</Text>
          ) : (
            <Text className="font-mono text-sm text-muted-foreground">
              {'• '.repeat(words.length).trim()}
            </Text>
          )}
          <View className="flex-row gap-2">
            <Button variant="outline" onPress={() => Clipboard.setString(mnemonic)}>
              <Text className="text-foreground">Copy</Text>
            </Button>
            <Button variant="outline" onPress={download}>
              <Text className="text-foreground">Download</Text>
            </Button>
          </View>
        </View>
      ) : (
        <Text className="text-sm text-muted-foreground">
          No kit is pending from this session. You can view or download your Emergency Kit anytime
          from Settings → Emergency Kit.
        </Text>
      )}
      <Button variant="outline" onPress={() => void router.navigate({ to: '/mfa' })}>
        <Text className="text-foreground">Open security settings</Text>
      </Button>
      <Button
        onPress={() => {
          services.auth.pendingRecoveryMnemonic = null;
          next();
        }}
      >
        <Text className="text-primary-foreground">Continue</Text>
      </Button>
    </View>
  );
}

function AddSecretStep() {
  return (
    <View className="gap-3">
      <Text className="text-xl font-semibold text-foreground">Add your first secret</Text>
      <Text className="text-sm text-muted-foreground">
        Open your vault and add a login, note, or card. Everything is encrypted before it leaves
        your device.
      </Text>
      <Button onPress={() => void router.navigate({ to: '/secrets' })}>
        <Text className="text-primary-foreground">Go to my vault</Text>
      </Button>
    </View>
  );
}

function DoneStep() {
  const { next } = useOnboarding();
  return (
    <View className="gap-2">
      <Text className="text-xl font-semibold text-foreground">You&apos;re all set</Text>
      <Text className="text-sm text-muted-foreground">
        That&apos;s the core loop: vault → Emergency Kit → secrets. You can replay this tour anytime
        from Settings.
      </Text>
      <Button onPress={() => void next()}>
        <Text className="text-primary-foreground">Finish</Text>
      </Button>
    </View>
  );
}

export const onboardingSteps: OnboardingStep[] = [
  {
    id: 'welcome',
    type: 'CUSTOM_COMPONENT',
    component: WelcomeStep,
    payload: { componentKey: 'welcome' },
    nextStep: 'create-vault',
  },
  {
    id: 'create-vault',
    type: 'CUSTOM_COMPONENT',
    component: CreateVaultStep,
    payload: { componentKey: 'create-vault' },
    previousStep: 'welcome',
    nextStep: 'emergency-kit',
  },
  {
    id: 'emergency-kit',
    type: 'CUSTOM_COMPONENT',
    component: EmergencyKitStep,
    payload: { componentKey: 'emergency-kit' },
    previousStep: 'create-vault',
    nextStep: 'add-secret',
    isSkippable: true,
    skipToStep: undefined,
  },
  {
    id: 'add-secret',
    type: 'CUSTOM_COMPONENT',
    component: AddSecretStep,
    payload: { componentKey: 'add-secret' },
    previousStep: 'emergency-kit',
    nextStep: 'done',
    isSkippable: true,
    skipToStep: undefined,
  },
  {
    id: 'done',
    type: 'CUSTOM_COMPONENT',
    component: DoneStep,
    payload: { componentKey: 'done' },
    previousStep: 'add-secret',
    nextStep: null,
  },
];

/**
 * Maps each `componentKey` to its React component. The onboarding engine does
 * not carry the component function through its (serializable) state, so steps
 * are resolved by key via this registry passed to `<OnboardingProvider
 * componentRegistry={...} />`.
 */
export const onboardingComponentRegistry: Record<string, StepComponent> = {
  welcome: WelcomeStep,
  'create-vault': CreateVaultStep,
  'emergency-kit': EmergencyKitStep,
  'add-secret': AddSecretStep,
  done: DoneStep,
};
