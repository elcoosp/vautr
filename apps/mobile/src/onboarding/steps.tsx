import { type OnboardingStep, useOnboarding } from '@onboardjs/react';
import { useNavigate } from '@tanstack/react-router';
import { useState } from 'react';
import { Text, TextInput, View } from 'react-native';
import { Button } from '../../components/ui/button';
import { services } from '../../lib/client';

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
  const navigate = useNavigate();
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async () => {
    setBusy(true);
    setError(null);
    try {
      const project = await services.api.createProject({ name: name.trim() || 'My Vault' });
      updateContext({ flowData: { vaultName: project.name } });
      navigate({ to: '/projects/$projectId', params: { projectId: project.uuid } });
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
  const navigate = useNavigate();
  return (
    <View className="gap-3">
      <Text className="text-xl font-semibold text-foreground">Save your Emergency Kit</Text>
      <Text className="text-sm text-muted-foreground">
        The Emergency Kit lets you recover your account if you forget your master password. It is
        generated and encrypted locally — store it somewhere safe (password manager, printed copy).
      </Text>
      <Button variant="outline" onPress={() => navigate({ to: '/mfa' })}>
        <Text className="text-foreground">Open security settings</Text>
      </Button>
    </View>
  );
}

function AddSecretStep() {
  const navigate = useNavigate();
  return (
    <View className="gap-3">
      <Text className="text-xl font-semibold text-foreground">Add your first secret</Text>
      <Text className="text-sm text-muted-foreground">
        Open your vault and add a login, note, or card. Everything is encrypted before it leaves
        your device.
      </Text>
      <Button onPress={() => navigate({ to: '/secrets' })}>
        <Text className="text-primary-foreground">Go to my vault</Text>
      </Button>
    </View>
  );
}

function DoneStep() {
  return (
    <View className="gap-2">
      <Text className="text-xl font-semibold text-foreground">You&apos;re all set</Text>
      <Text className="text-sm text-muted-foreground">
        That&apos;s the core loop: vault → Emergency Kit → secrets. You can replay this tour anytime
        from Settings.
      </Text>
    </View>
  );
}

export const onboardingSteps: OnboardingStep[] = [
  { id: 'welcome', type: 'CUSTOM_COMPONENT', component: WelcomeStep, nextStep: 'create-vault' },
  {
    id: 'create-vault',
    type: 'CUSTOM_COMPONENT',
    component: CreateVaultStep,
    previousStep: 'welcome',
    nextStep: 'emergency-kit',
  },
  {
    id: 'emergency-kit',
    type: 'CUSTOM_COMPONENT',
    component: EmergencyKitStep,
    previousStep: 'create-vault',
    nextStep: 'add-secret',
    isSkippable: true,
    skipToStep: undefined,
  },
  {
    id: 'add-secret',
    type: 'CUSTOM_COMPONENT',
    component: AddSecretStep,
    previousStep: 'emergency-kit',
    nextStep: 'done',
    isSkippable: true,
    skipToStep: undefined,
  },
  {
    id: 'done',
    type: 'CUSTOM_COMPONENT',
    component: DoneStep,
    previousStep: 'add-secret',
    nextStep: null,
  },
];
