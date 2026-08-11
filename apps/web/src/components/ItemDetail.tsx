import { useOverview } from '@vautr/ui-logic';
import { ArrowLeft, Copy, Eye, EyeOff } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { getClient } from '../lib/client';

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

  // Reveal on mount, dispose on unmount (zeroization contract).
  useEffect(() => {
    let active = true;
    void (async () => {
      try {
        const handle = await getClient().reveal(uuid, 1, new Uint8Array());
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
        void getClient().release(handleRef.current);
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
      await getClient().performAction({ type: 'CopyToClipboard', handle });
      setCopied(true);
      window.setTimeout(() => setCopied(false), 1500);
    } catch {
      // no-op in demo
    } finally {
      // Silent copy flow: immediately dispose the handle (ui-state-charts §3).
      void getClient().release(handle);
      handleRef.current = null;
    }
  };

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
    </div>
  );
}
