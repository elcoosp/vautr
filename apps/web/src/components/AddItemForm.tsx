import { evaluatePasswordStrength } from '@vautr/ui-logic';
import { useState } from 'react';
import { addItem, type NewItemInput } from '../lib/client';
import { Favicon } from './Favicon';

interface AddItemFormProps {
  onSaved: (uuid: string) => void;
  onCancel: () => void;
}

export function AddItemForm({ onSaved, onCancel }: AddItemFormProps) {
  const [title, setTitle] = useState('');
  const [username, setUsername] = useState('');
  const [password, setPassword] = useState('');
  const [url, setUrl] = useState('');
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const strength = evaluatePasswordStrength(password);

  const onSubmit = async (event: React.FormEvent) => {
    event.preventDefault();
    if (!title.trim() || !password) {
      setError('Title and password are required.');
      return;
    }
    setBusy(true);
    setError(null);
    try {
      const input: NewItemInput = {
        title: title.trim(),
        username: username.trim(),
        password,
        url: url.trim(),
      };
      const overview = await addItem(input);
      onSaved(overview.uuid);
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Could not save item.');
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={onSubmit} className="flex h-full flex-col bg-bg">
      <div className="flex items-center gap-2 border-b border-border px-4 py-3">
        <button
          type="button"
          onClick={onCancel}
          aria-label="Cancel add item"
          className="rounded-md p-1.5 text-text-muted hover:bg-surface-raised hover:text-text"
        >
          Cancel
        </button>
        <h2 className="text-lg font-semibold text-text">Add item</h2>
      </div>

      <div className="flex-1 space-y-4 overflow-auto p-4">
        <div>
          <label
            htmlFor="new-title"
            className="mb-1 block text-xs font-medium uppercase tracking-wide text-text-muted"
          >
            Title
          </label>
          <input
            id="new-title"
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
            className="w-full rounded-md border border-border bg-surface-raised px-3 py-2 text-text focus:border-accent"
            placeholder="e.g. Acme Bank"
          />
        </div>

        <div>
          <label
            htmlFor="new-username"
            className="mb-1 block text-xs font-medium uppercase tracking-wide text-text-muted"
          >
            Email or username
          </label>
          <input
            id="new-username"
            value={username}
            onChange={(e) => setUsername(e.target.value)}
            autoComplete="off"
            className="w-full rounded-md border border-border bg-surface-raised px-3 py-2 text-text focus:border-accent"
            placeholder="jane@example.com"
          />
        </div>

        <div>
          <label
            htmlFor="new-password"
            className="mb-1 block text-xs font-medium uppercase tracking-wide text-text-muted"
          >
            Password
          </label>
          <input
            id="new-password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            required
            className="w-full rounded-md border border-border bg-surface-raised px-3 py-2 text-text focus:border-accent"
            placeholder="••••••••"
          />
          {password ? (
            <div className="mt-1.5 space-y-1">
              <div className="flex items-center gap-2">
                <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-border">
                  <div
                    className={`h-full transition-all ${strength.score <= 1 ? 'bg-danger' : strength.score <= 3 ? 'bg-danger' : strength.score <= 4 ? 'bg-warn' : 'bg-accent'}`}
                    style={{ width: `${(strength.score / 7) * 100}%` }}
                  />
                </div>
                <span className={`text-xs font-medium ${strength.color}`}>{strength.label}</span>
              </div>
              {strength.suggestions.length > 0 ? (
                <p className="text-xs text-text-muted">{strength.suggestions[0]}</p>
              ) : null}
            </div>
          ) : null}
        </div>

        <div>
          <label
            htmlFor="new-url"
            className="mb-1 block text-xs font-medium uppercase tracking-wide text-text-muted"
          >
            Website
          </label>
          <div className="flex items-center gap-2">
            <Favicon url={url} size={20} className="shrink-0" />
            <input
              id="new-url"
              type="url"
              value={url}
              onChange={(e) => setUrl(e.target.value)}
              className="w-full rounded-md border border-border bg-surface-raised px-3 py-2 text-text focus:border-accent"
              placeholder="https://example.com"
            />
          </div>
        </div>

        {error ? (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        ) : null}
      </div>

      <div className="border-t border-border px-4 py-3">
        <button
          type="submit"
          disabled={busy}
          className="w-full rounded-md bg-accent px-4 py-2 font-medium text-accent-ink transition-opacity hover:opacity-90 disabled:opacity-50"
        >
          {busy ? 'Saving…' : 'Save item'}
        </button>
      </div>
    </form>
  );
}
