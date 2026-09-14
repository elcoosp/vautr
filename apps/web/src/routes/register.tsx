import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useIsLocked } from '@vautr/ui-logic';

import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { login, register } from '@/lib/client';

export const Route = createFileRoute('/register')({
  component: RegisterPage,
});

function RegisterPage() {
  const isLocked = useIsLocked();
  const navigate = useNavigate();
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [confirm, setConfirm] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

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
    if (password !== confirm) {
      setError('Passwords do not match.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const { recoveryMnemonic } = await register(username.trim(), password);
      // Hand the freshly-generated Recovery Key to the onboarding flow so it can
      // be shown once. Stored in sessionStorage (cleared on tab close) — never
      // persisted in plaintext; the durable copy is KEK-sealed in IndexedDB.
      sessionStorage.setItem('vautr:pending-kit', recoveryMnemonic);
      await login(username.trim(), password);
      void navigate({ to: '/dashboard' });
    } catch (err) {
      // WebKit fetch failures surface as `Error` with an EMPTY `.message`
      // (e.g. server unreachable / cross-origin). Never render a blank error.
      const msg =
        err instanceof Error && err.message.trim()
          ? err.message.trim()
          : 'Registration failed. Make sure the Vautr server is running at http://localhost:8080.';
      setError(msg);
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
          <p className="mb-5 text-sm text-text-muted">Create a new zero-knowledge vault.</p>
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
                autoComplete="new-password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                placeholder="Choose a strong master password"
              />
            </div>
            <div className="space-y-1.5">
              <Label htmlFor="confirm-password">Confirm master password</Label>
              <Input
                id="confirm-password"
                type="password"
                autoComplete="new-password"
                value={confirm}
                onChange={(e) => setConfirm(e.target.value)}
                placeholder="Repeat your master password"
              />
            </div>
            {error ? (
              <p role="alert" className="text-sm text-danger">
                {error}
              </p>
            ) : null}
            <Button type="submit" className="vault-btn-press w-full" disabled={busy}>
              {busy ? 'Creating…' : 'Create vault'}
            </Button>
          </form>
        </div>

        <p className="text-center text-xs text-text-muted">
          Already have an account?{' '}
          <Link to="/login" className="text-accent underline-offset-2 hover:underline">
            Log in
          </Link>
        </p>
      </div>
    </main>
  );
}
