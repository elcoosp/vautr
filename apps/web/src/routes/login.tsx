import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useIsLocked } from '@vautr/ui-logic';

import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { login, MfaRequiredError, completeLoginWithTotp, recoverWithKit } from '@/lib/client';

export const Route = createFileRoute('/login')({
  component: LoginPage,
});

function LoginPage() {
  const isLocked = useIsLocked();
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  // VTRFIX-SEC-C03: when the server demands a second factor, we park the
  // pending token and render the TOTP step. The master password stays in
  // component state (never persisted) so we can unwrap the SVK after the
  // server mints the session.
  const [pendingToken, setPendingToken] = useState<string | null>(null);
  // VTRFIX-FEAT-H02: emergency recovery flow state.
  const [recoverMode, setRecoverMode] = useState(false);
  const [recoverMnemonic, setRecoverMnemonic] = useState('');
  const [recoverNewPassword, setRecoverNewPassword] = useState('');
  const [recoverNewMnemonic, setRecoverNewMnemonic] = useState<string | null>(null);
  const [totpCode, setTotpCode] = useState('');

  useEffect(() => {
    if (!isLocked) {
      void navigate({ to: '/dashboard' });
    }
  }, [isLocked, navigate]);

  const onSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !password) {
      setError('Enter your username and master password.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await login(username.trim(), password);
      void navigate({ to: '/dashboard' });
    } catch (err) {
      if (err instanceof MfaRequiredError) {
        // Server withheld the session; render the TOTP step. Password stays
        // in state so the follow-up call can unwrap the SVK locally.
        setPendingToken(err.pendingToken);
      } else {
        setError(err instanceof Error ? err.message : 'Login failed.');
      }
    } finally {
      setBusy(false);
    }
  };

  // VTRFIX-SEC-C03: complete the login by submitting the TOTP code. The
  // pending token is single-use and expires in 5 minutes.
  const onSubmitTotp = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!pendingToken || !totpCode.trim()) {
      setError('Enter the 6-digit code from your authenticator app.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await completeLoginWithTotp(pendingToken, totpCode.trim(), username.trim(), password);
      void navigate({ to: '/dashboard' });
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Invalid one-time code.');
    } finally {
      setBusy(false);
    }
  };

  const cancelTotp = () => {
    setPendingToken(null);
    setTotpCode('');
    setError(null);
  };

  // VTRFIX-FEAT-H02: submit the recovery kit + new password.
  const onRecover = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!username.trim() || !recoverMnemonic.trim() || !recoverNewPassword) {
      setError('Enter username, recovery kit, and a new master password.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const { newMnemonic } = await recoverWithKit(
        username.trim(),
        recoverMnemonic.trim(),
        recoverNewPassword,
      );
      setRecoverNewMnemonic(newMnemonic);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Recovery failed.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="animate-vault-enter w-full max-w-sm space-y-8">
        {/* Brand header */}
        <div className="space-y-2 text-center">
          <div className="mx-auto grid size-16 place-items-center rounded-2xl bg-surface ring-1 ring-border">
            <img src="/logo.svg" alt="Vautr" className="size-10" />
          </div>
          <h1 className="text-2xl font-semibold tracking-tight text-text">Vautr</h1>
          <p className="text-sm text-text-muted">Zero-knowledge vault</p>
        </div>

        {/* Form card */}
        <div className="rounded-xl border border-border bg-surface p-6">
          <p className="mb-5 text-sm text-text-muted">Unlock your vault to continue.</p>
          {pendingToken ? (
            <form onSubmit={onSubmitTotp} className="space-y-4">
              <p className="text-sm text-text-muted">
                Enter the 6-digit code from your authenticator app.
              </p>
              <div className="space-y-1.5">
                <Label htmlFor="totp-code">One-time code</Label>
                <Input
                  id="totp-code"
                  type="text"
                  inputMode="numeric"
                  autoComplete="one-time-code"
                  value={totpCode}
                  onChange={(e) => setTotpCode(e.target.value)}
                  placeholder="123456"
                  autoFocus
                />
              </div>
              {error ? (
                <p role="alert" className="text-sm text-danger">
                  {error}
                </p>
              ) : null}
              <Button type="submit" className="vault-btn-press w-full" disabled={busy}>
                {busy ? 'Verifying\u2026' : 'Verify and unlock'}
              </Button>
              <Button
                type="button"
                variant="ghost"
                className="w-full"
                disabled={busy}
                onClick={cancelTotp}
              >
                Use a different account
              </Button>
            </form>
          ) : (
          <form onSubmit={onSubmit} className="space-y-4">
            <div className="space-y-1.5">
              <Label htmlFor="username">Username</Label>
              <Input
                id="username"
                type="text"
                autoComplete="username"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="you@example.com"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="master-password">Master password</Label>
              <Input
                id="master-password"
                type="password"
                autoComplete="current-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Enter your master password"
              />
            </div>
            {error ? (
              <p role="alert" className="text-sm text-danger">
                {error}
              </p>
            ) : null}
            <Button type="submit" className="vault-btn-press w-full" disabled={busy}>
              {busy ? 'Unlocking…' : 'Unlock vault'}
            </Button>
          </form>
          )}
        </div>

        <p className="text-center text-xs text-text-muted">
          New here?{' '}
          <Link to="/register" className="text-accent underline-offset-2 hover:underline">
            Register an account
          </Link>
          {' · '}
          <button
            type="button"
            onClick={() => setRecoverMode(true)}
            className="text-accent underline-offset-2 hover:underline"
          >
            Recover with kit
          </button>
        </p>

        {/* VTRFIX-FEAT-H02: emergency recovery form + result view. */}
        {recoverMode ? (
          <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
            <div className="w-full max-w-md rounded-xl border border-border bg-surface p-6">
              {recoverNewMnemonic ? (
                <>
                  <h2 className="mb-2 text-lg font-semibold">New Recovery Kit</h2>
                  <p className="mb-3 text-sm text-text-muted">
                    Write these 24 words down. The old kit is now unusable.
                  </p>
                  <pre className="mb-4 whitespace-pre-wrap rounded border border-border bg-surface-raised p-3 font-mono text-xs">
                    {recoverNewMnemonic}
                  </pre>
                  <Button
                    className="w-full"
                    onClick={() => {
                      void navigator.clipboard?.writeText(recoverNewMnemonic);
                      setRecoverMode(false);
                      setRecoverNewMnemonic(null);
                      setRecoverMnemonic('');
                      setRecoverNewPassword('');
                    }}
                  >
                    Copy and close
                  </Button>
                </>
              ) : (
                <form onSubmit={onRecover} className="space-y-4">
                  <h2 className="text-lg font-semibold">Recover with Emergency Kit</h2>
                  <p className="text-sm text-text-muted">
                    Paste the 24-word kit you saved at registration and choose a new
                    master password. Your vault contents are preserved.
                  </p>
                  <div className="space-y-1.5">
                    <Label htmlFor="recover-mnemonic">Recovery kit</Label>
                    <textarea
                      id="recover-mnemonic"
                      value={recoverMnemonic}
                      onChange={(e) => setRecoverMnemonic(e.target.value)}
                      rows={3}
                      className="w-full rounded-md border border-border bg-surface-raised px-3 py-2 font-mono text-xs"
                      placeholder="word1 word2 … word24"
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor="recover-new-password">New master password</Label>
                    <Input
                      id="recover-new-password"
                      type="password"
                      autoComplete="new-password"
                      value={recoverNewPassword}
                      onChange={(e) => setRecoverNewPassword(e.target.value)}
                    />
                  </div>
                  {error ? (
                    <p role="alert" className="text-sm text-danger">
                      {error}
                    </p>
                  ) : null}
                  <Button type="submit" className="w-full" disabled={busy}>
                    {busy ? 'Recovering…' : 'Recover vault'}
                  </Button>
                  <Button
                    type="button"
                    variant="ghost"
                    className="w-full"
                    disabled={busy}
                    onClick={() => {
                      setRecoverMode(false);
                      setError(null);
                    }}
                  >
                    Cancel
                  </Button>
                </form>
              )}
            </div>
          </div>
        ) : null}
      </div>
    </main>
  );
}
