import { type OnboardingStep, useOnboarding } from '@onboardjs/react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { getPopupMlpClient } from '@/popup/popupClient';
import { usePopupStore } from '@/popup/store';

/**
 * First-run guided onboarding steps (browser extension). Each step renders its
 * own content; OnboardingFlow supplies the Back / Next / Skip chrome. Steps drive
 * real client surfaces (create vault, switch tabs) — no stubs.
 */

function WelcomeStep() {
  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">Welcome to Vautr</h2>
      <p className="text-sm text-muted-foreground">
        Vautr is a zero-knowledge vault: your secrets are encrypted on your device and the server
        never sees them. Let&apos;s set up the essentials in about a minute.
      </p>
    </div>
  );
}

function CreateVaultStep() {
  const { next, updateContext, state } = useOnboarding();
  const setActiveTab = usePopupStore((s) => s.setActiveTab);
  const [name, setName] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async () => {
    setBusy(true);
    setError(null);
    try {
      const mlp = await getPopupMlpClient();
      const project = await mlp.createProject({ name: name.trim() || 'My Vault' });
      updateContext({ flowData: { vaultName: project.name } });
      setActiveTab('projects');
      next();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not create vault.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">Create your first vault</h2>
      <p className="text-sm text-muted-foreground">
        A vault (project) groups your secrets. You can create more later.
      </p>
      <div className="space-y-1.5">
        <Label htmlFor="onboard-vault-name">Vault name</Label>
        <Input
          id="onboard-vault-name"
          placeholder="My Vault"
          value={state?.context.flowData.vaultName ?? name}
          onChange={(e) => setName(e.target.value)}
        />
      </div>
      {error ? <p className="text-sm text-destructive">{error}</p> : null}
      <Button onClick={onSubmit} disabled={busy} className="w-full">
        {busy ? 'Creating…' : 'Create vault & continue'}
      </Button>
    </div>
  );
}

function EmergencyKitStep() {
  const setActiveTab = usePopupStore((s) => s.setActiveTab);
  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">Save your Emergency Kit</h2>
      <p className="text-sm text-muted-foreground">
        The Emergency Kit lets you recover your account if you forget your master password. It is
        generated and encrypted locally — store it somewhere safe.
      </p>
      <Button variant="outline" onClick={() => setActiveTab('mfa')} className="w-full">
        Open MFA &amp; security
      </Button>
    </div>
  );
}

function AddSecretStep() {
  const setActiveTab = usePopupStore((s) => s.setActiveTab);
  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">Add your first secret</h2>
      <p className="text-sm text-muted-foreground">
        Open your vault and add a login, note, or card. Everything is encrypted before it leaves
        your device.
      </p>
      <Button onClick={() => setActiveTab('secrets')} className="w-full">
        Go to secrets
      </Button>
    </div>
  );
}

function DoneStep() {
  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">You&apos;re all set</h2>
      <p className="text-sm text-muted-foreground">
        That&apos;s the core loop: vault → Emergency Kit → secrets. You can replay this tour anytime
        from the popup footer.
      </p>
    </div>
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
