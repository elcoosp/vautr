import { useIsReadOnly, useVaultActions } from '@vautr/ui-logic';
import { useState } from 'react';
import { login } from '../lib/client';

/**
 * Persistent, non-blocking read-only gate banner (ui-state-charts §2). Viewing
 * and copying stay enabled; mutations are disabled. Re-authenticating re-enables
 * mutations.
 */
export function ReadOnlyBanner() {
  const isReadOnly = useIsReadOnly();
  const [reauth, setReauth] = useState(false);
  const [password, setPassword] = useState('');
  const { unlock } = useVaultActions();

  if (!isReadOnly) {
    return null;
  }

  const onReauth = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!password) {
      return;
    }
    const username = localStorage.getItem('vautr:username') ?? '';
    await login(username, password);
    unlock();
    setReauth(false);
    setPassword('');
  };

  return (
    <div
      role="status"
      aria-live="polite"
      className="border-b border-warn/30 bg-warn/10 px-4 py-3 text-sm text-warn"
    >
      <div className="mx-auto flex max-w-5xl flex-wrap items-center justify-between gap-3">
        <span className="font-medium">Vault updated. Re-authenticate to save changes.</span>
        {reauth ? (
          <form onSubmit={onReauth} className="flex items-center gap-2">
            <label htmlFor="reauth-password" className="sr-only">
              Master password
            </label>
            <input
              id="reauth-password"
              type="password"
              autoComplete="current-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              className="rounded-md border border-border bg-surface-raised px-3 py-1.5 text-text"
              placeholder="Master password"
            />
            <button
              type="submit"
              className="rounded-md bg-warn px-3 py-1.5 font-medium text-accent-ink"
            >
              Re-authenticate
            </button>
          </form>
        ) : (
          <button
            type="button"
            onClick={() => setReauth(true)}
            className="rounded-md border border-warn px-3 py-1.5 font-medium text-warn hover:bg-warn/10"
          >
            Re-authenticate
          </button>
        )}
      </div>
    </div>
  );
}
