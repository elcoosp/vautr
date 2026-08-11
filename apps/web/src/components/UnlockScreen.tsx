import { useState } from 'react';
import { login, register } from '../lib/client';

type Mode = 'login' | 'register';

export function UnlockScreen() {
  const [mode, setMode] = useState<Mode>('login');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!username.trim() || !password) {
      setError('Enter your username and master password.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      if (mode === 'register') {
        await register(username.trim(), password);
        // Account created; log straight in with the same password.
        await login(username.trim(), password);
      } else {
        await login(username.trim(), password);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Authentication failed.');
    } finally {
      setBusy(false);
    }
  };

  const switchMode = (next: Mode) => {
    setMode(next);
    setError(null);
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="w-full max-w-sm rounded-xl border border-border bg-surface p-8">
        <h1 className="mb-2 text-2xl font-semibold text-text">Vautr</h1>
        <p className="mb-6 text-sm text-text-muted">
          {mode === 'login'
            ? 'Unlock your vault to view saved items.'
            : 'Create a new zero-knowledge vault.'}
        </p>

        <div className="mb-4 grid grid-cols-2 gap-1 rounded-md bg-surface-raised p-1" role="tablist">
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'login'}
            onClick={() => switchMode('login')}
            className={`rounded px-3 py-1.5 text-sm font-medium transition-colors ${
              mode === 'login' ? 'bg-accent text-accent-ink' : 'text-text-muted hover:text-text'
            }`}
          >
            Log in
          </button>
          <button
            type="button"
            role="tab"
            aria-selected={mode === 'register'}
            onClick={() => switchMode('register')}
            className={`rounded px-3 py-1.5 text-sm font-medium transition-colors ${
              mode === 'register' ? 'bg-accent text-accent-ink' : 'text-text-muted hover:text-text'
            }`}
          >
            Register
          </button>
        </div>

        <form onSubmit={onSubmit}>
          <label htmlFor="username" className="mb-2 block text-sm font-medium text-text">
            Username
          </label>
          <input
            id="username"
            type="text"
            autoComplete="username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            className="mb-4 w-full rounded-md border border-border bg-surface-raised px-3 py-2 text-text focus:border-accent"
            placeholder="you@example.com"
          />

          <label htmlFor="master-password" className="mb-2 block text-sm font-medium text-text">
            Master password
          </label>
          <input
            id="master-password"
            type="password"
            autoComplete={mode === 'login' ? 'current-password' : 'new-password'}
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className="mb-4 w-full rounded-md border border-border bg-surface-raised px-3 py-2 text-text focus:border-accent"
            placeholder="••••••••"
          />

          {error ? (
            <p role="alert" className="mb-4 text-sm text-danger">
              {error}
            </p>
          ) : null}

          <button
            type="submit"
            disabled={busy}
            className="w-full rounded-md bg-accent px-4 py-2 font-medium text-accent-ink transition-opacity hover:opacity-90 disabled:opacity-50"
          >
            {busy
              ? mode === 'login'
                ? 'Unlocking…'
                : 'Creating…'
              : mode === 'login'
                ? 'Unlock vault'
                : 'Create vault'}
          </button>
        </form>

        <p className="mt-4 text-center text-xs text-text-muted">
          {mode === 'login' ? (
            <>
              New here?{' '}
              <button
                type="button"
                onClick={() => switchMode('register')}
                className="text-accent underline"
              >
                Register an account
              </button>
            </>
          ) : (
            <>
              Already have an account?{' '}
              <button
                type="button"
                onClick={() => switchMode('login')}
                className="text-accent underline"
              >
                Log in
              </button>
            </>
          )}
        </p>
      </div>
    </main>
  );
}
