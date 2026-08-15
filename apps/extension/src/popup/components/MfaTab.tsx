import type { MfaStatus } from '@vautr/api-contract';
import type { VautrMlpClient } from '@vautr/client-sdk';
import type { VautrWebClient } from '@vautr/client-sdk/real';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { renderKitHtml } from '@/lib/kit';

interface MfaTabProps {
  mlp: VautrMlpClient;
  client: VautrWebClient;
}

export function MfaTab({ mlp, client }: MfaTabProps) {
  const [status, setStatus] = useState<MfaStatus | null>(null);
  const [issue, setIssue] = useState<{
    otpauth_url: string;
    secret: string;
    enrollment_id: string;
  } | null>(null);
  const [code, setCode] = useState('');
  const [recoveryCodes, setRecoveryCodes] = useState<string[]>([]);
  const [error, setError] = useState('');

  const [keyGen, setKeyGen] = useState<number | null>(null);
  const [rotatePassword, setRotatePassword] = useState('');
  const [rotating, setRotating] = useState(false);
  const [kitRevealed, setKitRevealed] = useState(false);

  const kit = client.getEmergencyKit();
  const downloadKit = () => {
    if (!kit) return;
    const email = sessionStorage.getItem('vautr:username') || 'you';
    const html = renderKitHtml(kit.mnemonic, email);
    const blob = new Blob([html], { type: 'text/html' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'vautr-emergency-kit.html';
    a.click();
    URL.revokeObjectURL(url);
  };

  useEffect(() => {
    setKeyGen(client.getKeyGen());
  }, [client]);

  async function handleRotate(): Promise<void> {
    if (!rotatePassword) {
      setError('Enter your master password to rotate the key.');
      return;
    }
    setRotating(true);
    setError('');
    try {
      const newGen = await client.rotateKey(rotatePassword);
      setKeyGen(newGen);
      setRotatePassword('');
      toast.success(`Vault key rotated to generation ${newGen}.`);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setRotating(false);
    }
  }

  const refresh = useCallback(async (): Promise<void> => {
    try {
      const res = await mlp.mfaStatus();
      setStatus(res);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }, [mlp]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  async function handleIssue(): Promise<void> {
    setError('');
    try {
      const res = await mlp.totpIssue();
      setIssue({
        otpauth_url: res.otpauth_url,
        secret: res.secret,
        enrollment_id: res.enrollment_id,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  async function handleVerify(): Promise<void> {
    if (!issue || !code) return;
    setError('');
    try {
      const res = await mlp.totpVerify({ enrollment_id: issue.enrollment_id, code });
      if (res.recovery_codes?.length) {
        setRecoveryCodes(res.recovery_codes);
      }
      toast.success('TOTP verified and enabled.');
      setIssue(null);
      setCode('');
      await refresh();
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    }
  }

  const methods = status?.configured_methods ?? [];

  return (
    <div className="space-y-4">
      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Two-factor authentication</CardTitle>
          <CardDescription className="text-xs">
            {status
              ? status.required
                ? 'MFA is required for your account.'
                : 'MFA is optional for your account.'
              : 'Loading status…'}
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-4">
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-xs text-muted-foreground">Configured:</span>
            {methods.length === 0 ? (
              <Badge variant="outline">None</Badge>
            ) : (
              methods.map((m) => (
                <Badge key={m} variant="secondary">
                  {m}
                </Badge>
              ))
            )}
          </div>

          {error ? <p className="text-sm text-destructive">{error}</p> : null}

          {!issue && !recoveryCodes.length ? (
            <Button variant="outline" onClick={() => void handleIssue()}>
              Set up authenticator app (TOTP)
            </Button>
          ) : null}

          {issue ? (
            <div className="space-y-3 rounded border p-3">
              <p className="text-xs text-muted-foreground">
                Scan the QR code or add the secret to your authenticator app, then enter the code to
                confirm.
              </p>
              <div className="space-y-1">
                <Label>Setup key</Label>
                <Input readOnly value={issue.secret} className="font-mono" />
              </div>
              <div className="space-y-1">
                <Label>6-digit code</Label>
                <Input value={code} onChange={(e) => setCode(e.target.value)} />
              </div>
              <Button onClick={() => void handleVerify()}>Verify and enable</Button>
              <div className="space-y-1">
                <Label>OTP auth URL</Label>
                <Input readOnly value={issue.otpauth_url} className="text-xs break-all" />
              </div>
            </div>
          ) : null}

          {recoveryCodes.length ? (
            <div className="space-y-2 rounded border p-3">
              <p className="text-xs font-medium">Recovery codes (save these):</p>
              <div className="grid grid-cols-2 gap-1">
                {recoveryCodes.map((c) => (
                  <code key={c} className="rounded bg-muted px-1.5 py-0.5 text-xs">
                    {c}
                  </code>
                ))}
              </div>
            </div>
          ) : null}

          <div className="flex items-center justify-between">
            <Label htmlFor="mfa-required" className="cursor-pointer">
              MFA required for this account
            </Label>
            <Switch
              id="mfa-required"
              checked={status?.required ?? false}
              onCheckedChange={async (v) => {
                await mlp.updateMfaPolicy({ required: v });
                await refresh();
              }}
            />
          </div>
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Emergency Kit</CardTitle>
          <CardDescription className="text-xs">
            Your Recovery Key recovers this account if you forget your master password. It is stored
            encrypted on this device and never sent to the server.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          {kit ? (
            <>
              <div className="flex items-center justify-between">
                <span className="text-xs text-muted-foreground">24-word Recovery Key</span>
                <Button variant="ghost" size="sm" onClick={() => setKitRevealed((v) => !v)}>
                  {kitRevealed ? 'Hide' : 'Reveal'}
                </Button>
              </div>
              {kitRevealed ? (
                <p className="select-all break-words rounded border border-border bg-background p-2 font-mono text-xs">
                  {kit.mnemonic}
                </p>
              ) : (
                <p className="font-mono text-xs text-muted-foreground">
                  {'• '.repeat(kit.words.length).trim()}
                </p>
              )}
              <div className="flex flex-wrap gap-2">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void navigator.clipboard.writeText(kit.mnemonic)}
                >
                  Copy
                </Button>
                <Button variant="outline" size="sm" onClick={downloadKit}>
                  Download
                </Button>
              </div>
            </>
          ) : (
            <p className="text-xs text-muted-foreground">
              No Emergency Kit found for this account. If you registered before kits were enabled,
              you can generate one by rotating your recovery key.
            </p>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader>
          <CardTitle className="text-sm">Vault key rotation</CardTitle>
          <CardDescription className="text-xs">
            Re-wraps your vault key under a new generation. Enter your master password to confirm.
          </CardDescription>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="flex items-center justify-between text-xs text-muted-foreground">
            <span>Current key generation</span>
            <span className="font-mono">{keyGen ?? '—'}</span>
          </div>
          <div className="space-y-1">
            <Label htmlFor="ext-rotate-mp">Master password</Label>
            <Input
              id="ext-rotate-mp"
              type="password"
              value={rotatePassword}
              onChange={(e) => setRotatePassword(e.target.value)}
              placeholder="Confirm to rotate"
            />
          </div>
          {error ? <p className="text-xs text-destructive">{error}</p> : null}
          <Button
            variant="outline"
            onClick={() => void handleRotate()}
            disabled={rotating || !client.isUnlocked()}
          >
            {rotating ? 'Rotating…' : 'Rotate vault key'}
          </Button>
        </CardContent>
      </Card>
    </div>
  );
}
