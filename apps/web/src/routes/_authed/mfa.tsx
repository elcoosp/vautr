import { createFileRoute } from '@tanstack/react-router';
import type { MfaMethod, MfaPolicy, MfaStatus } from '@vautr/api-contract';
import { totpCode } from '@vautr/ui-logic';
import { ShieldCheck, Smartphone } from 'lucide-react';
import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { MlpApiError, mlp } from '@/lib/mlp';

export const Route = createFileRoute('/_authed/mfa')({
  component: MfaPage,
});

function MfaPage() {
  const [status, setStatus] = useState<MfaStatus | null>(null);
  const [_policy, setPolicy] = useState<MfaPolicy | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);
  const [enrolling, setEnrolling] = useState(false);
  const [enrollment, setEnrollment] = useState<{
    enrollment_id: string;
    secret: string;
    qr_code_data_url: string;
    otpauth_url: string;
  } | null>(null);
  const [liveCode, setLiveCode] = useState<string>('');
  const [recoveryCodes, setRecoveryCodes] = useState<string[] | null>(null);

  // Policy edit state
  const [polRequired, setPolRequired] = useState(false);
  const [polMethods, setPolMethods] = useState<string[]>(['totp']);
  const [polMinLength, setPolMinLength] = useState(12);
  const [polUpper, setPolUpper] = useState(true);
  const [polLower, setPolLower] = useState(true);
  const [polDigit, setPolDigit] = useState(true);
  const [polSpecial, setPolSpecial] = useState(true);
  const [polEntropy, setPolEntropy] = useState(60);
  const [policyBusy, setPolicyBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const [s, p] = await Promise.all([mlp.mfaStatus(), mlp.mfaPolicyGet()]);
      setStatus(s);
      setPolicy(p);
      setPolRequired(!!p.required);
      setPolMethods(p.allowed_methods ?? ['totp']);
      const mp = p.master_password_policy ?? {};
      setPolMinLength(mp.min_length ?? 12);
      setPolUpper(mp.require_upper ?? true);
      setPolLower(mp.require_lower ?? true);
      setPolDigit(mp.require_digit ?? true);
      setPolSpecial(mp.require_special ?? true);
      setPolEntropy(mp.min_entropy_bits ?? 60);
      setError(null);
    } catch (err) {
      setError(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const startEnroll = async () => {
    setEnrolling(true);
    setRecoveryCodes(null);
    try {
      const res = await mlp.mfaTotpIssue();
      setEnrollment({
        enrollment_id: res.enrollment_id,
        secret: res.secret,
        qr_code_data_url: res.qr_code_data_url ?? '',
        otpauth_url: res.otpauth_url,
      });
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setEnrolling(false);
    }
  };

  // Keep a live TOTP code ticking while an enrollment is open.
  useEffect(() => {
    if (!enrollment) return;
    let active = true;
    const tick = async () => {
      try {
        const code = await totpCode(enrollment.otpauth_url);
        if (active) setLiveCode(code);
      } catch {
        if (active) setLiveCode('');
      }
    };
    void tick();
    const id = setInterval(tick, 1000);
    return () => {
      active = false;
      clearInterval(id);
    };
  }, [enrollment]);

  const verifyEnroll = async () => {
    if (!enrollment || !liveCode) return;
    try {
      const res = await mlp.mfaTotpVerify({
        enrollment_id: enrollment.enrollment_id,
        code: liveCode,
      });
      toast.success('TOTP enabled');
      setRecoveryCodes(res.recovery_codes ?? null);
      setEnrollment(null);
      void load();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    }
  };

  const savePolicy = async () => {
    setPolicyBusy(true);
    try {
      const res = await mlp.mfaPolicyUpdate({
        required: polRequired,
        allowed_methods: polMethods as MfaMethod[],
        master_password_policy: {
          min_length: polMinLength,
          require_upper: polUpper,
          require_lower: polLower,
          require_digit: polDigit,
          require_special: polSpecial,
          min_entropy_bits: polEntropy,
        },
      });
      setPolicy(res);
      toast.success('Policy saved');
      void load();
    } catch (err) {
      toast.error(err instanceof MlpApiError ? err.message : String(err));
    } finally {
      setPolicyBusy(false);
    }
  };

  const toggleMethod = (method: string) => {
    setPolMethods((prev) =>
      prev.includes(method) ? prev.filter((m) => m !== method) : [...prev, method],
    );
  };

  if (loading) return <p className="p-6 text-sm text-text-muted">Loading…</p>;
  if (error && !status) return <p className="p-6 text-sm text-danger">{error}</p>;

  return (
    <div className="space-y-6 p-6">
      <div>
        <h1 className="text-2xl font-semibold text-text">MFA &amp; security</h1>
        <p className="text-sm text-text-muted">
          Manage multi-factor authentication and organization policy.
        </p>
      </div>

      <div className="grid gap-6 lg:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle className="flex items-center gap-2">
              <Smartphone className="size-4 text-accent" aria-hidden="true" />
              Authenticator app (TOTP)
            </CardTitle>
            <CardDescription>
              {status?.configured_methods.includes('totp')
                ? 'TOTP is enabled on your account.'
                : 'TOTP is not configured yet.'}
            </CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center gap-2">
              <ShieldCheck className="size-4 text-text-muted" aria-hidden="true" />
              <span className="text-sm text-text">Configured methods:</span>
              <div className="flex gap-1">
                {(status?.configured_methods ?? []).length === 0 ? (
                  <Badge variant="outline">none</Badge>
                ) : (
                  status?.configured_methods.map((m) => <Badge key={m}>{m}</Badge>)
                )}
              </div>
            </div>

            {enrollment ? (
              <div className="space-y-3 rounded-md border border-border bg-surface-raised p-4">
                <p className="text-sm font-medium text-text">
                  Scan or enter the secret in your authenticator app
                </p>
                <div className="flex justify-center">
                  {enrollment.qr_code_data_url ? (
                    <img
                      src={enrollment.qr_code_data_url}
                      alt="TOTP QR code"
                      className="h-36 w-36 rounded-md bg-white p-1"
                    />
                  ) : null}
                </div>
                <div className="flex items-center justify-between gap-2">
                  <code className="rounded bg-bg px-2 py-1 font-mono text-xs text-text">
                    {enrollment.secret}
                  </code>
                  <span className="font-mono text-2xl tracking-widest text-accent">{liveCode}</span>
                </div>
                <Button className="w-full" onClick={() => void verifyEnroll()} disabled={!liveCode}>
                  Verify &amp; enable
                </Button>
              </div>
            ) : (
              <Button
                onClick={() => void startEnroll()}
                disabled={enrolling || status?.configured_methods.includes('totp')}
              >
                {enrolling ? 'Preparing…' : 'Set up authenticator app'}
              </Button>
            )}

            {recoveryCodes ? (
              <div className="rounded-md border border-warn/40 bg-warn/10 p-4">
                <p className="mb-2 text-sm font-semibold text-warn">Save your recovery codes</p>
                <div className="grid grid-cols-2 gap-1 font-mono text-sm text-text">
                  {recoveryCodes.map((c) => (
                    <span key={c}>{c}</span>
                  ))}
                </div>
              </div>
            ) : null}
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Organization policy</CardTitle>
            <CardDescription>Enforce MFA and master-password strength for members.</CardDescription>
          </CardHeader>
          <CardContent className="space-y-4">
            <div className="flex items-center justify-between rounded-md border border-border bg-surface-raised px-3 py-2">
              <Label className="cursor-pointer text-sm text-text">
                Require MFA for all members
              </Label>
              <Switch
                checked={polRequired}
                onCheckedChange={setPolRequired}
                aria-label="Require MFA"
              />
            </div>
            <div className="space-y-1.5">
              <Label>Allowed methods</Label>
              <div className="grid gap-2">
                {['totp', 'webauthn', 'email'].map((m) => (
                  <label
                    key={m}
                    htmlFor={`pol-method-${m}`}
                    className="flex items-center gap-2 text-sm text-text"
                  >
                    <Checkbox
                      id={`pol-method-${m}`}
                      checked={polMethods.includes(m)}
                      onCheckedChange={() => toggleMethod(m)}
                    />
                    {m}
                  </label>
                ))}
              </div>
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="pol-minlen">Minimum password length: {polMinLength}</Label>
              <Input
                id="pol-minlen"
                type="range"
                min={8}
                max={32}
                value={polMinLength}
                onChange={(e) => setPolMinLength(Number(e.target.value))}
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="pol-entropy">Minimum entropy bits: {polEntropy}</Label>
              <Input
                id="pol-entropy"
                type="range"
                min={20}
                max={120}
                value={polEntropy}
                onChange={(e) => setPolEntropy(Number(e.target.value))}
              />
            </div>
            <div className="grid grid-cols-2 gap-2">
              {[
                { label: 'Require uppercase', value: polUpper, set: setPolUpper },
                { label: 'Require lowercase', value: polLower, set: setPolLower },
                { label: 'Require digit', value: polDigit, set: setPolDigit },
                { label: 'Require special', value: polSpecial, set: setPolSpecial },
              ].map(({ label, value, set }) => (
                <label
                  key={label}
                  htmlFor={`pol-${label.replace(/\s+/g, '-').toLowerCase()}`}
                  className="flex items-center gap-2 rounded-md border border-border bg-surface-raised px-3 py-2 text-sm text-text"
                >
                  <Checkbox
                    id={`pol-${label.replace(/\s+/g, '-').toLowerCase()}`}
                    checked={value}
                    onCheckedChange={(v) => set(!!v)}
                  />
                  {label}
                </label>
              ))}
            </div>
            <Button
              className="w-full"
              onClick={() => void savePolicy()}
              disabled={policyBusy || polMethods.length === 0}
            >
              {policyBusy ? 'Saving…' : 'Save policy'}
            </Button>
          </CardContent>
        </Card>
      </div>
    </div>
  );
}
