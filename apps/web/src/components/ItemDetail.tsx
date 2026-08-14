import { useOverview } from '@vautr/ui-logic';
import { ArrowLeft, Copy, Eye, EyeOff, Share2 } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import {
  ensureSharingKey,
  getItemPlaintext,
  performAction,
  release,
  reveal,
  shareItem,
} from '../lib/client';

interface ItemDetailProps {
  uuid: string;
  onBack: () => void;
}

/**
 * Item detail view. Follows the opaque-handle lifecycle (ui-state-charts §3):
 * the handle lives in a `useRef` (never global state), copy is delegated via
 * `perform_action`, and the handle is explicitly released on unmount.
 */
export function ItemDetail({ uuid, onBack }: ItemDetailProps) {
  const overview = useOverview(uuid);
  const handleRef = useRef<string | null>(null);
  const [masked, setMasked] = useState(true);
  const [copied, setCopied] = useState(false);
  const [shareOpen, setShareOpen] = useState(false);
  const [shareRecipient, setShareRecipient] = useState('');
  const [shareBusy, setShareBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Reveal on mount, dispose on unmount (zeroization contract).
  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const handle = await reveal(uuid);
        if (active) {
          handleRef.current = handle;
        }
      } catch {
        // ignore: copy will surface the error.
      }
    })();
    return () => {
      active = false;
      if (handleRef.current) {
        void release(handleRef.current);
        handleRef.current = null;
      }
    };
  }, [uuid]);

  if (!overview) {
    return (
      <p className="p-6 text-sm text-text-muted">
        Item not found.{' '}
        <button type="button" className="text-accent underline" onClick={onBack}>
          Back to vault
        </button>
      </p>
    );
  }

  const onCopy = async () => {
    const handle = handleRef.current;
    if (!handle) {
      return;
    }
    try {
      await performAction({ type: 'CopyToClipboard', handle });
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // no-op
    } finally {
      // Silent copy flow: immediately dispose the handle (ui-state-charts §3).
      void release(handle);
      handleRef.current = null;
    }
  };

  async function confirmShare(): Promise<void> {
    if (!shareRecipient) {
      setError('Recipient user id is required.');
      return;
    }
    setShareBusy(true);
    setError(null);
    try {
      await ensureSharingKey();
      const plaintext = await getItemPlaintext(uuid);
      await shareItem(uuid, shareRecipient, plaintext);
      setShareOpen(false);
      setShareRecipient('');
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Failed to share item.');
    } finally {
      setShareBusy(false);
    }
  }

  return (
    <div className="flex h-full flex-col bg-bg">
      <div className="flex items-center gap-2 border-b border-border px-4 py-3">
        <button
          type="button"
          onClick={onBack}
          aria-label="Back to vault list"
          className="rounded-md p-1.5 text-text-muted hover:bg-surface-raised hover:text-text"
        >
          <ArrowLeft className="size-5" aria-hidden="true" />
        </button>
        <h2 className="truncate text-lg font-semibold text-text">{overview.title}</h2>
      </div>

      <div className="flex-1 overflow-auto p-4">
        <dl className="space-y-4">
          <div>
            <dt className="mb-1 text-xs font-medium uppercase tracking-wide text-text-muted">
              Username
            </dt>
            <dd className="text-text">{overview.subtitle}</dd>
          </div>

          <div>
            <dt className="mb-1 text-xs font-medium uppercase tracking-wide text-text-muted">
              Password
            </dt>
            <dd className="flex items-center gap-2">
              <span
                aria-live="polite"
                className="flex-1 rounded-md border border-border bg-surface-raised px-3 py-2 font-mono text-text"
              >
                {masked ? '••••••••••' : '••••••••••'}
              </span>
              <button
                type="button"
                onClick={() => setMasked((value) => !value)}
                aria-label={masked ? 'Show password' : 'Hide password'}
                className="rounded-md border border-border p-2 text-text-muted hover:bg-surface-raised hover:text-text"
              >
                {masked ? (
                  <Eye className="size-4" aria-hidden="true" />
                ) : (
                  <EyeOff className="size-4" aria-hidden="true" />
                )}
              </button>
              <button
                type="button"
                onClick={() => void onCopy()}
                aria-label={`Copy password for ${overview.title}`}
                className="rounded-md bg-accent px-3 py-2 font-medium text-accent-ink hover:opacity-90"
              >
                <span className="inline-flex items-center gap-1.5">
                  <Copy className="size-4" aria-hidden="true" />
                  {copied ? 'Copied' : 'Copy'}
                </span>
              </button>
              <button
                type="button"
                onClick={() => setShareOpen(true)}
                aria-label={`Share ${overview.title}`}
                className="rounded-md border border-border px-3 py-2 font-medium text-text hover:bg-surface-raised"
              >
                <span className="inline-flex items-center gap-1.5">
                  <Share2 className="size-4" aria-hidden="true" />
                  Share
                </span>
              </button>
            </dd>
          </div>

          {overview.urls.length > 0 ? (
            <div>
              <dt className="mb-1 text-xs font-medium uppercase tracking-wide text-text-muted">
                Website
              </dt>
              <dd>
                <a
                  href={overview.urls[0]}
                  className="text-accent underline"
                  rel="noreferrer"
                  target="_blank"
                >
                  {overview.urls[0]}
                </a>
              </dd>
            </div>
          ) : null}
        </dl>
      </div>

      {shareOpen ? (
        <div
          role="dialog"
          aria-modal="true"
          aria-label={`Share ${overview.title}`}
          className="absolute inset-0 z-10 flex items-center justify-center bg-black/40 p-4"
          onClick={(e) => {
            if (e.target === e.currentTarget) setShareOpen(false);
          }}
          onKeyDown={(e) => {
            if (e.key === 'Escape') setShareOpen(false);
          }}
        >
          <div className="w-full max-w-sm space-y-3 rounded-lg border border-border bg-surface p-4 shadow-lg">
            <h3 className="text-sm font-semibold text-text">Share “{overview.title}”</h3>
            <p className="text-xs text-text-muted">
              Encrypts the item under a one-time key and delivers it to the recipient’s inbox. The
              server only ever stores ciphertext.
            </p>
            <label className="block text-xs font-medium text-text-muted" htmlFor="share-recipient">
              Recipient user id
            </label>
            <input
              id="share-recipient"
              value={shareRecipient}
              onChange={(e) => setShareRecipient(e.target.value)}
              placeholder="recipient username"
              className="w-full rounded-md border border-border bg-bg px-3 py-2 text-text"
            />
            {error ? <p className="text-xs text-destructive">{error}</p> : null}
            <div className="flex justify-end gap-2">
              <button
                type="button"
                onClick={() => setShareOpen(false)}
                className="rounded-md border border-border px-3 py-1.5 text-sm text-text hover:bg-surface-raised"
              >
                Cancel
              </button>
              <button
                type="button"
                onClick={() => void confirmShare()}
                disabled={shareBusy}
                className="rounded-md bg-accent px-3 py-1.5 text-sm font-medium text-accent-ink hover:opacity-90 disabled:opacity-50"
              >
                {shareBusy ? 'Sharing…' : 'Share'}
              </button>
            </div>
          </div>
        </div>
      ) : null}
    </div>
  );
}
