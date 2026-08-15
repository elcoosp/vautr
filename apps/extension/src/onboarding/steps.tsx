import { type OnboardingStep, useOnboarding } from '@onboardjs/react';
import { useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { renderKitHtml } from '@/lib/kit';
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
  const { next } = useOnboarding();
  const [revealed, setRevealed] = useState(false);
  const mnemonic =
    typeof sessionStorage !== 'undefined' ? sessionStorage.getItem('vautr:pending-kit') : null;
  const words = mnemonic ? mnemonic.split(/\s+/).filter(Boolean) : [];

  const download = () => {
    if (!mnemonic) return;
    const email = sessionStorage.getItem('vautr:username') || 'you';
    const html = renderKitHtml(mnemonic, email);
    const blob = new Blob([html], { type: 'text/html' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'vautr-emergency-kit.html';
    a.click();
    URL.revokeObjectURL(url);
    sessionStorage.removeItem('vautr:pending-kit');
  };

  return (
    <div className="space-y-3">
      <h2 className="text-lg font-semibold">Save your Emergency Kit</h2>
      <p className="text-sm text-muted-foreground">
        Your Recovery Key is the only way to recover your account if you forget your master
        password. It was generated and encrypted on this device. Store it somewhere safe.
      </p>
      {mnemonic ? (
        <div className="space-y-2 rounded-lg border border-border bg-background p-3">
          <div className="flex items-center justify-between">
            <span className="text-xs font-medium text-muted-foreground">24-word Recovery Key</span>
            <Button variant="ghost" size="sm" onClick={() => setRevealed((v) => !v)}>
              {revealed ? 'Hide' : 'Reveal'}
            </Button>
          </div>
          {revealed ? (
            <p className="select-all break-words font-mono text-sm">{mnemonic}</p>
          ) : (
            <p className="font-mono text-sm text-muted-foreground">
              {'• '.repeat(words.length).trim()}
            </p>
          )}
          <div className="flex gap-2">
            <Button
              variant="outline"
              size="sm"
              onClick={() => void navigator.clipboard.writeText(mnemonic)}
            >
              Copy
            </Button>
            <Button variant="outline" size="sm" onClick={download}>
              Download
            </Button>
          </div>
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          No kit is pending from this session. You can view or download your Emergency Kit anytime
          from MFA &amp; security.
        </p>
      )}
      <Button variant="outline" onClick={() => setActiveTab('mfa')} className="w-full">
        Open MFA &amp; security
      </Button>
      <Button
        onClick={() => {
          sessionStorage.removeItem('vautr:pending-kit');
          next();
        }}
        className="w-full"
      >
        Continue
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
