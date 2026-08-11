import { useEffect, useState } from 'react';
import * as browser from 'webextension-polyfill';
import type { DecryptedOverview } from '@vautr/ui-logic';
import { useIsLocked, useOverviews, vaultEventBus } from '@vautr/ui-logic';
import {
  createItemCiphertextStore,
  createSvkSessionStore,
  type AutofillResponse,
} from '@vautr/client-sdk/extension';
import { localArea, sessionArea } from '../lib/extensionStorage';
import { DEMO_CIPHERTEXTS, DEMO_ITEMS, makeDemoKey } from './demo';
import { disposePopupClient, getPopupClient } from './popupClient';

const AUTOFILL_REQUEST = 'VAUTR_AUTOFILL';

const styles = {
  shell: { padding: 16, display: 'flex', flexDirection: 'column' as const, gap: 12 },
  heading: { margin: 0, fontSize: 18, fontWeight: 700 },
  label: { fontSize: 12, color: '#6b7280', marginBottom: 4, display: 'block' },
  input: {
    width: '100%',
    boxSizing: 'border-box' as const,
    padding: '8px 10px',
    borderRadius: 8,
    border: '1px solid #d1d5db',
    fontSize: 14,
  },
  button: {
    width: '100%',
    padding: '8px 12px',
    borderRadius: 8,
    border: 'none',
    background: '#4f46e5',
    color: '#fff',
    fontSize: 14,
    fontWeight: 600,
    cursor: 'pointer',
  },
  item: {
    padding: 12,
    borderRadius: 8,
    background: '#fff',
    border: '1px solid #e5e7eb',
  },
  itemTitle: { margin: 0, fontSize: 14, fontWeight: 600 },
  itemSubtitle: { margin: 0, fontSize: 12, color: '#6b7280' },
  row: { display: 'flex', gap: 8, marginTop: 8 },
  action: {
    flex: 1,
    padding: '5px 8px',
    borderRadius: 6,
    border: '1px solid #d1d5db',
    background: '#fff',
    fontSize: 12,
    cursor: 'pointer',
  },
  notice: { fontSize: 12, color: '#374151', whiteSpace: 'pre-wrap' as const },
} as const;

export function App() {
  const isLocked = useIsLocked();
  const overviews = useOverviews();
  const [password, setPassword] = useState('');
  const [busy, setBusy] = useState(false);
  const [notice, setNotice] = useState('');

  // Graceful shutdown of the popup-scoped worker when the popup closes.
  useEffect(() => {
    const shutdown = (): void => disposePopupClient();
    window.addEventListener('pagehide', shutdown);
    window.addEventListener('beforeunload', shutdown);
    return () => {
      window.removeEventListener('pagehide', shutdown);
      window.removeEventListener('beforeunload', shutdown);
    };
  }, []);

  async function handleUnlock(): Promise<void> {
    setBusy(true);
    try {
      const client = getPopupClient();
      const key = makeDemoKey(password);
      await client.unlock(key, 1);

      // Cache the raw SVK in browser-encrypted session storage (build-env-deploy §3.3).
      const svkStore = createSvkSessionStore(sessionArea);
      await svkStore.cache(key);

      for (const item of DEMO_ITEMS) {
        vaultEventBus.emit({ type: 'OverviewUpserted', overview: item });
      }
      setNotice('Unlocked. Vault ready for autofill.');
    } catch (error) {
      setNotice(`Unlock failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBusy(false);
    }
  }

  async function handleAutofill(item: DecryptedOverview): Promise<void> {
    const ct = DEMO_CIPHERTEXTS[item.uuid];
    if (!ct) {
      setNotice('No cached ciphertext for this item.');
      return;
    }
    // Cache the item ciphertext so the stateless SW can decrypt it (local, never plaintext).
    const ciphertextStore = createItemCiphertextStore(localArea);
    await ciphertextStore.cache({ uuid: item.uuid, encKeyGen: ct.encKeyGen, payload: ct.payload });

    const response = (await browser.runtime.sendMessage({
      type: AUTOFILL_REQUEST,
      uuid: item.uuid,
    })) as AutofillResponse | undefined;

    if (response && response.ok && response.filled) {
      setNotice(`Autofilled "${item.title}".`);
    } else if (response && response.ok) {
      setNotice(`Decrypted "${item.title}" but no focused field found.`);
    } else {
      setNotice(`Autofill failed: ${response?.error ?? 'unhandled'}`);
    }
  }

  async function handleCopy(item: DecryptedOverview): Promise<void> {
    const ct = DEMO_CIPHERTEXTS[item.uuid];
    if (!ct) {
      setNotice('No cached ciphertext for this item.');
      return;
    }
    const client = getPopupClient();
    const handle = await client.reveal(item.uuid, ct.encKeyGen, new Uint8Array(ct.payload));
    await client.performAction({ type: 'CopyToClipboard', handle });
    setNotice(`Copied "${item.title}" to clipboard.`);
  }

  return (
    <div style={styles.shell}>
      <h1 style={styles.heading}>Vautr</h1>

      {isLocked ? (
        <div>
          <label style={styles.label} htmlFor="vault-password">
            Master password
          </label>
          <input
            id="vault-password"
            style={styles.input}
            type="password"
            value={password}
            disabled={busy}
            onChange={(e) => setPassword(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === 'Enter') {
                void handleUnlock();
              }
            }}
          />
          <button style={{ ...styles.button, marginTop: 10 }} disabled={busy} onClick={() => void handleUnlock()}>
            Unlock
          </button>
        </div>
      ) : (
        <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
          {overviews.map((item) => (
            <div key={item.uuid} style={styles.item}>
              <p style={styles.itemTitle}>{item.title}</p>
              <p style={styles.itemSubtitle}>{item.subtitle}</p>
              <div style={styles.row}>
                <button style={styles.action} onClick={() => void handleAutofill(item)}>
                  Autofill
                </button>
                <button style={styles.action} onClick={() => void handleCopy(item)}>
                  Copy
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {notice ? <div style={styles.notice}>{notice}</div> : null}
    </div>
  );
}
