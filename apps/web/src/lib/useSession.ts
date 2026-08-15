import { IndexedDbStore } from '@vautr/client-sdk/storage';
import { useEffect, useState } from 'react';

/**
 * Reactive "is the user logged in" signal for the web client.
 *
 * The session bearer lives in the shared IndexedDB store (`@vautr/client-sdk`),
 * not in React state, so we poll it on mount and after the relevant lifecycle
 * events (login writes the token; logout/register-forget clears it). Onboarding
 * and other post-auth surfaces must only render when this returns true, because
 * the server rejects API calls made without a valid session token.
 */
const store = new IndexedDbStore();

export function useSession(): boolean {
  const [authed, setAuthed] = useState(false);

  useEffect(() => {
    let alive = true;

    const refresh = async () => {
      const state = await store.getState();
      if (!alive) return;
      setAuthed(!!state.sessionToken);
    };

    void refresh();

    // Re-check after focus (e.g. returning to the tab) and on a custom
    // auth-change event the client fires on login/logout.
    const onChange = () => void refresh();
    window.addEventListener('focus', onChange);
    window.addEventListener('vautr:auth-change', onChange);

    return () => {
      alive = false;
      window.removeEventListener('focus', onChange);
      window.removeEventListener('vautr:auth-change', onChange);
    };
  }, []);

  return authed;
}
