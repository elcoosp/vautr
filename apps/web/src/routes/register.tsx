import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useIsLocked } from '@vautr/ui-logic';
import { Lock } from 'lucide-react';
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
      <div className="w-full max-w-sm rounded-xl border border-border bg-surface p-8">
        <div className="mb-6 flex items-center gap-2">
          <span className="grid size-9 place-items-center rounded-lg bg-accent/15 text-accent">
            <Lock className="size-4" aria-hidden="true" />
          </span>
          <h1 className="text-2xl font-semibold text-text">Vautr</h1>
        </div>
        <p className="mb-6 text-sm text-text-muted">Create a new zero-knowledge vault.</p>
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
              placeholder="••••••••"
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
              placeholder="••••••••"
            />
          </div>
          {error ? (
            <p role="alert" className="text-sm text-danger">
              {error}
            </p>
          ) : null}
          <Button type="submit" className="w-full" disabled={busy}>
            {busy ? 'Creating…' : 'Create vault'}
          </Button>
        </form>
        <p className="mt-4 text-center text-xs text-text-muted">
          Already have an account?{' '}
          <Link to="/login" className="text-accent underline">
            Log in
          </Link>
        </p>
      </div>
    </main>
  );
}
