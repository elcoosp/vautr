import { useVaultActions } from '@vautr/ui-logic';
import { useState } from 'react';
import { getClient, makeDemoKey, seedDemoData } from '../lib/client';

export function UnlockScreen() {
  const [password, setPassword] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const { unlock } = useVaultActions();

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!password) {
      setError('Enter a master password to continue.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await getClient().unlock(makeDemoKey(password), 1);
      seedDemoData();
      unlock();
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unlock failed.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <main className="flex min-h-screen items-center justify-center bg-bg p-6">
      <div className="w-full max-w-sm rounded-xl border border-border bg-surface p-8">
        <h1 className="mb-2 text-2xl font-semibold text-text">Vautr</h1>
        <p className="mb-6 text-sm text-text-muted">Unlock your vault to view saved items.</p>
        <form onSubmit={onSubmit}>
          <label htmlFor="master-password" className="mb-2 block text-sm font-medium text-text">
            Master password
          </label>
          <input
            id="master-password"
            type="password"
            autoComplete="current-password"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
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
            {busy ? 'Unlocking…' : 'Unlock vault'}
          </button>
        </form>
      </div>
    </main>
  );
}
