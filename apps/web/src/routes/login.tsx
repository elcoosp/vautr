import { createFileRoute, Link, useNavigate } from '@tanstack/react-router';
import { useIsLocked } from '@vautr/ui-logic';

import { useEffect, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { login } from '@/lib/client';

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
      setError(err instanceof Error ? err.message : 'Login failed.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="w-full max-w-sm rounded-xl border border-border bg-surface p-8">
        <div className="mb-6 flex items-center gap-3">
          <img src="/logo.svg" alt="Vautr" className="size-20 rounded-lg" />
          <h1 className="text-2xl font-semibold text-text">Vautr</h1>
        </div>
        <p className="mb-6 text-sm text-text-muted">Unlock your vault to continue.</p>
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
              placeholder="••••••••"
            />
          </div>
          {error ? (
            <p role="alert" className="text-sm text-danger">
              {error}
            </p>
          ) : null}
          <Button type="submit" className="w-full" disabled={busy}>
            {busy ? 'Unlocking…' : 'Unlock vault'}
          </Button>
        </form>
        <p className="mt-4 text-center text-xs text-text-muted">
          New here?{' '}
          <Link to="/register" className="text-accent underline">
            Register an account
          </Link>
        </p>
      </div>
    </main>
  );
}
